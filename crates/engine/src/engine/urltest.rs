use std::time::{Duration, Instant};

use tokio::sync::watch;
use tokio::time::{sleep, timeout};
use tracing::{debug, info, warn};
use zero_config::{OutboundGroupConfig, OutboundGroupKind};
use zero_core::{Address, Network, ProtocolType, Session};
use zero_traits::AsyncSocket;

use super::error::EngineError;
use super::resolve::ResolvedLeafOutbound;
use super::runtime::Engine;

impl Engine {
    pub(crate) async fn run_urltest_group(
        &self,
        group: OutboundGroupConfig,
        mut shutdown: watch::Receiver<bool>,
    ) -> Result<(), EngineError> {
        let OutboundGroupKind::UrlTest {
            outbounds,
            url,
            interval_seconds,
        } = &group.group
        else {
            return Ok(());
        };

        let probe =
            UrlTestProbe::parse(url).map_err(|message| EngineError::InvalidUrlTestGroup {
                tag: group.tag.clone(),
                message,
            })?;

        info!(
            group_tag = %group.tag,
            url = probe.url.as_str(),
            interval_seconds = *interval_seconds,
            "urltest group started"
        );

        loop {
            self.refresh_urltest_group(group.tag(), outbounds, &probe)
                .await;

            tokio::select! {
                changed = shutdown.changed() => {
                    match changed {
                        Ok(()) if *shutdown.borrow() => break,
                        Ok(()) => {}
                        Err(_) => break,
                    }
                }
                _ = sleep(Duration::from_secs(*interval_seconds)) => {}
            }
        }

        info!(group_tag = %group.tag, "urltest group stopped");
        Ok(())
    }

    async fn refresh_urltest_group(
        &self,
        group_tag: &str,
        members: &[String],
        probe: &UrlTestProbe,
    ) {
        let mut best: Option<ProbeSuccess> = None;

        for member in members {
            let Some(outbound) = self
                .config
                .outbounds
                .iter()
                .find(|outbound| outbound.tag() == member)
            else {
                continue;
            };

            let candidate = super::resolve::resolve_named_outbound(outbound);
            match self.probe_outbound(candidate, probe).await {
                Ok(latency_ms) => {
                    if best
                        .as_ref()
                        .map(|current| latency_ms < current.latency_ms)
                        .unwrap_or(true)
                    {
                        best = Some(ProbeSuccess {
                            outbound_tag: member.clone(),
                            latency_ms,
                        });
                    }
                }
                Err(error) => {
                    debug!(
                        group_tag = group_tag,
                        outbound_tag = member,
                        error = %error,
                        "urltest probe failed"
                    );
                }
            }
        }

        let previous = self.outbound_group_state.selected_outbound(group_tag);
        let Some(selected) = best
            .as_ref()
            .map(|probe| probe.outbound_tag.as_str())
            .or(previous.as_deref())
            .or_else(|| members.first().map(String::as_str))
        else {
            return;
        };

        let latency_ms = best
            .as_ref()
            .and_then(|probe| (probe.outbound_tag == selected).then_some(probe.latency_ms));
        self.outbound_group_state
            .update_urltest(group_tag, selected, latency_ms);

        match previous.as_deref() {
            Some(previous) if previous == selected => debug!(
                group_tag = group_tag,
                selected = selected,
                latency_ms = latency_ms,
                "urltest group refreshed"
            ),
            _ => info!(
                group_tag = group_tag,
                selected = selected,
                latency_ms = latency_ms,
                "urltest group selected outbound"
            ),
        }

        if best.is_none() {
            warn!(
                group_tag = group_tag,
                selected = selected,
                "urltest probe found no healthy outbound; keeping current selection"
            );
        }
    }

    async fn probe_outbound(
        &self,
        candidate: ResolvedLeafOutbound<'_>,
        probe: &UrlTestProbe,
    ) -> Result<u64, EngineError> {
        const URLTEST_PROBE_TIMEOUT: Duration = Duration::from_secs(5);

        timeout(URLTEST_PROBE_TIMEOUT, async {
            let started_at = Instant::now();

            let mut socket = match candidate {
                ResolvedLeafOutbound::Direct { .. } => self
                    .protocols
                    .direct_outbound
                    .connect_host(probe.host.as_str(), probe.port, &self.resolver)
                    .await
                    .map_err(EngineError::from)?,
                ResolvedLeafOutbound::Block { .. } => {
                    return Err(std::io::Error::other("block outbound is not probeable").into());
                }
                ResolvedLeafOutbound::Socks5 { server, port, .. } => {
                    let session = Session::new(
                        0,
                        Address::Domain(probe.host.clone()),
                        probe.port,
                        Network::Tcp,
                        ProtocolType::Unknown,
                    );
                    self.connect_via_socks5_upstream(&session, server, port)
                        .await?
                }
            };

            socket
                .write_all(probe.request.as_bytes())
                .await
                .map_err(EngineError::from)?;

            let mut buf = [0_u8; 1];
            let read = socket.read(&mut buf).await.map_err(EngineError::from)?;
            if read == 0 {
                return Err(std::io::Error::other(
                    "probe target closed connection without response",
                )
                .into());
            }

            Ok(started_at.elapsed().as_millis() as u64)
        })
        .await
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::TimedOut, "urltest probe timed out"))?
    }
}

struct ProbeSuccess {
    outbound_tag: String,
    latency_ms: u64,
}

struct UrlTestProbe {
    url: String,
    host: String,
    port: u16,
    request: String,
}

impl UrlTestProbe {
    fn parse(url: &str) -> Result<Self, String> {
        let rest = url
            .strip_prefix("http://")
            .ok_or_else(|| "urltest currently only supports `http://` probe urls".to_owned())?;

        let (authority, path) = match rest.split_once('/') {
            Some((authority, suffix)) => (authority, format!("/{}", suffix)),
            None => (rest, "/".to_owned()),
        };

        if authority.trim().is_empty() {
            return Err("urltest probe url requires a host".to_owned());
        }

        let (host, port) = parse_authority(authority)?;
        let host_header = if port == 80 {
            authority.to_owned()
        } else if authority.contains(':') && !authority.starts_with('[') {
            format!("{host}:{port}")
        } else {
            authority.to_owned()
        };

        let request =
            format!("HEAD {path} HTTP/1.1\r\nHost: {host_header}\r\nConnection: close\r\n\r\n");

        Ok(Self {
            url: url.to_owned(),
            host,
            port,
            request,
        })
    }
}

fn parse_authority(authority: &str) -> Result<(String, u16), String> {
    if let Some(rest) = authority.strip_prefix('[') {
        let (host, port_part) = rest
            .split_once(']')
            .ok_or_else(|| "invalid bracketed host in urltest probe url".to_owned())?;
        let port = match port_part.strip_prefix(':') {
            Some(port) => port
                .parse::<u16>()
                .map_err(|_| "invalid port in urltest probe url".to_owned())?,
            None if port_part.is_empty() => 80,
            _ => return Err("invalid authority in urltest probe url".to_owned()),
        };
        return Ok((host.to_owned(), port));
    }

    match authority.rsplit_once(':') {
        Some((host, port)) if !host.contains(':') => Ok((
            host.to_owned(),
            port.parse::<u16>()
                .map_err(|_| "invalid port in urltest probe url".to_owned())?,
        )),
        _ => Ok((authority.to_owned(), 80)),
    }
}
