use std::time::{Duration, Instant};

use tokio::sync::watch;
use tokio::time::{sleep, timeout};
use tracing::{debug, info, warn};
use zero_core::{Address, Network, ProtocolType, Session};
use zero_traits::AsyncSocket;

use super::super::runtime::upstream::VlessUpstream;
use super::super::transport::TcpRelayStream;
use super::super::{logging::log_urltest_group_target_changed, runtime::Proxy};
use zero_engine::{
    EngineError, ResolvedLeafOutbound, ResolvedOutbound, TargetId, TargetKind, UrlTestMemberState,
};

impl Proxy {
    pub(crate) async fn run_urltest_group(
        &self,
        group_id: TargetId,
        mut shutdown: watch::Receiver<bool>,
    ) -> Result<(), EngineError> {
        let group = self
            .engine()
            .plan()
            .target(group_id)
            .expect("engine plan should resolve urltest group");
        let TargetKind::UrlTest(urltest) = group.kind() else {
            return Ok(());
        };
        let probe = UrlTestProbe::parse(urltest.url()).map_err(|message| {
            EngineError::InvalidUrlTestGroup {
                tag: group.tag().to_owned(),
                message,
            }
        })?;

        info!(
            group_tag = %group.tag(),
            url = probe.url.as_str(),
            interval_seconds = urltest.interval().as_secs(),
            "urltest group started"
        );

        loop {
            self.refresh_urltest_group(group_id, &probe).await;

            tokio::select! {
                changed = shutdown.changed() => {
                    match changed {
                        Ok(()) if *shutdown.borrow() => break,
                        Ok(()) => {}
                        Err(_) => break,
                    }
                }
                _ = sleep(urltest.interval()) => {}
            }
        }

        info!(group_tag = %group.tag(), "urltest group stopped");
        Ok(())
    }

    async fn refresh_urltest_group(&self, group_id: TargetId, probe: &UrlTestProbe) {
        let group = self
            .engine()
            .plan()
            .target(group_id)
            .expect("engine plan should resolve urltest group");
        let TargetKind::UrlTest(urltest) = group.kind() else {
            return;
        };
        let group_tag = group.tag();
        let mut best: Option<ProbeSuccess> = None;
        let checked_at_unix_ms = unix_timestamp_ms();
        let mut member_states = Vec::with_capacity(urltest.members().len());

        for member_id in urltest.members() {
            let member = self.target_tag(*member_id).unwrap_or("<unknown>");
            let effective_chains = self.resolve_target_chains(*member_id);
            let Some(candidate) = self.resolve_target_id(*member_id) else {
                member_states.push(UrlTestMemberState {
                    member_id: *member_id,
                    healthy: false,
                    latency_ms: None,
                    last_checked_unix_ms: Some(checked_at_unix_ms),
                    last_error: Some("failed to resolve probe target".to_owned()),
                    effective_chains,
                });
                continue;
            };

            match self.probe_outbound(candidate, probe).await {
                Ok(latency_ms) => {
                    if best
                        .as_ref()
                        .map(|current| latency_ms < current.latency_ms)
                        .unwrap_or(true)
                    {
                        best = Some(ProbeSuccess {
                            outbound_id: *member_id,
                            latency_ms,
                        });
                    }

                    member_states.push(UrlTestMemberState {
                        member_id: *member_id,
                        healthy: true,
                        latency_ms: Some(latency_ms),
                        last_checked_unix_ms: Some(checked_at_unix_ms),
                        last_error: None,
                        effective_chains,
                    });
                }
                Err(error) => {
                    debug!(
                        group_tag = group_tag,
                        outbound_tag = member,
                        error = %error,
                        "urltest probe failed"
                    );
                    member_states.push(UrlTestMemberState {
                        member_id: *member_id,
                        healthy: false,
                        latency_ms: None,
                        last_checked_unix_ms: Some(checked_at_unix_ms),
                        last_error: Some(error.to_string()),
                        effective_chains,
                    });
                }
            }
        }

        let previous = self.urltest_selected_target(group_id);
        let Some(selected) = best
            .as_ref()
            .map(|probe| probe.outbound_id)
            .or(previous)
            .or(Some(urltest.initial_member()))
        else {
            return;
        };
        let selected_tag = self.target_tag(selected).unwrap_or("<unknown>");
        let previous_tag = previous.and_then(|target| self.target_tag(target));

        let latency_ms = best
            .as_ref()
            .and_then(|probe| (probe.outbound_id == selected).then_some(probe.latency_ms));
        self.update_urltest_state(group_id, selected, latency_ms, member_states);
        log_urltest_group_target_changed(group_tag, previous_tag, selected_tag, latency_ms);

        if best.is_none() {
            warn!(
                group_tag = group_tag,
                selected = selected_tag,
                "urltest probe found no healthy outbound; keeping current selection"
            );
        }
    }

    async fn probe_outbound(
        &self,
        candidate: ResolvedOutbound<'_>,
        probe: &UrlTestProbe,
    ) -> Result<u64, EngineError> {
        match candidate {
            ResolvedOutbound::Single(candidate) => self.probe_leaf_outbound(candidate, probe).await,
            ResolvedOutbound::Fallback { candidates } => {
                let mut last_error = None;

                for candidate in candidates {
                    match self.probe_leaf_outbound(candidate, probe).await {
                        Ok(latency_ms) => return Ok(latency_ms),
                        Err(error) => last_error = Some(error),
                    }
                }

                Err(last_error
                    .expect("validated fallback groups always have at least one candidate"))
            }
        }
    }

    async fn probe_leaf_outbound(
        &self,
        candidate: ResolvedLeafOutbound<'_>,
        probe: &UrlTestProbe,
    ) -> Result<u64, EngineError> {
        const URLTEST_PROBE_TIMEOUT: Duration = Duration::from_secs(5);

        timeout(URLTEST_PROBE_TIMEOUT, async {
            let started_at = Instant::now();

            let mut socket: TcpRelayStream = match candidate {
                ResolvedLeafOutbound::Direct { .. } => self
                    .protocols
                    .direct_outbound
                    .connect_host(probe.host.as_str(), probe.port, &self.resolver)
                    .await
                    .map_err(EngineError::from)?
                    .into(),
                ResolvedLeafOutbound::Block { .. } => {
                    return Err(std::io::Error::other("block outbound is not probeable").into());
                }
                ResolvedLeafOutbound::Socks5 {
                    server,
                    port,
                    username,
                    password,
                    ..
                } => {
                    let session = Session::new(
                        0,
                        Address::Domain(probe.host.clone()),
                        probe.port,
                        Network::Tcp,
                        ProtocolType::Unknown,
                    );
                    self.connect_via_socks5_upstream(&session, server, port, username.zip(password))
                        .await?
                }
                ResolvedLeafOutbound::Vless {
                    server,
                    port,
                    id,
                    flow,
                    tls,
                    reality,
                    ws,
                    ..
                } => {
                    let session = Session::new(
                        0,
                        Address::Domain(probe.host.clone()),
                        probe.port,
                        Network::Tcp,
                        ProtocolType::Unknown,
                    );
                    self.connect_via_vless_upstream(
                        &session,
                        VlessUpstream {
                            server,
                            port,
                            id,
                            flow,
                            mux_concurrency: None,
                            mux_idle_timeout_secs: None,
                            tls,
                            reality,
                            ws,
                            grpc: None,
                            h2: None,
                            http_upgrade: None,
                            quic: None,
                        },
                    )
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

fn unix_timestamp_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_millis() as u64
}

struct ProbeSuccess {
    outbound_id: TargetId,
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
