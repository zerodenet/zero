use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{watch, Notify};
use tokio::time::{sleep, timeout};
use tracing::{debug, info, warn};
use zero_core::{Address, Network, ProtocolType, Session};
use zero_traits::AsyncSocket;

use super::super::transport::extract_tcp_stream;
use super::super::{logging::log_urltest_group_target_changed, runtime::Proxy};
use zero_engine::{
    EngineError, PolicyProbeCompletedPayload, PolicyProbeMember, ProbeTrigger,
    ResolvedLeafOutbound, ResolvedOutbound, TargetId, UrlTestMemberState,
};

/// Default probe URL for single-outbound diagnostics (`diagnostics.probe_outbound`).
/// Plain HTTP so the measured latency excludes a TLS handshake, and a 204
/// response so there is no body to download — the de-facto standard also used
/// by Clash/sing-box.
pub const DEFAULT_PROBE_URL: &str = "http://www.gstatic.com/generate_204";

impl Proxy {
    pub(crate) async fn run_urltest_group(
        &self,
        group_id: TargetId,
        mut shutdown: watch::Receiver<bool>,
    ) -> Result<(), EngineError> {
        let plan = self.engine().plan();
        let group = plan
            .target(group_id)
            .expect("engine plan should resolve urltest group");
        let Some(urltest) = group.as_urltest() else {
            return Ok(());
        };
        let group_tag = group.tag().to_owned();
        let probe = UrlTestProbe::parse(urltest.url()).map_err(|message| {
            EngineError::InvalidUrlTestGroup {
                tag: group_tag.clone(),
                message,
            }
        })?;

        // Register a probe trigger so `policies.probe` can wake this loop.
        let probe_notify = Arc::new(Notify::new());
        let trigger = ProbeTrigger::new({
            let notify = Arc::clone(&probe_notify);
            move || notify.notify_one()
        });
        self.engine()
            .probe_trigger_registry()
            .register(&group_tag, trigger);

        info!(
            group_tag = %group_tag,
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
                _ = probe_notify.notified() => {
                    debug!(group_tag = %group_tag, "urltest probe triggered by api");
                }
                _ = sleep(urltest.interval()) => {}
            }
        }

        self.engine().probe_trigger_registry().remove(&group_tag);
        info!(group_tag = %group_tag, "urltest group stopped");
        Ok(())
    }

    async fn refresh_urltest_group(&self, group_id: TargetId, probe: &UrlTestProbe) {
        let plan = self.engine().plan();
        let group = plan
            .target(group_id)
            .expect("engine plan should resolve urltest group");
        let Some(urltest) = group.as_urltest() else {
            return;
        };
        let group_tag = group.tag();
        let mut best: Option<ProbeSuccess> = None;
        let checked_at_unix_ms = unix_timestamp_ms();
        let mut member_states = Vec::with_capacity(urltest.members().len());

        for member_id in urltest.members() {
            let member = self
                .target_tag(*member_id)
                .unwrap_or_else(|| "<unknown>".to_owned());
            let effective_chains = self.resolve_target_chains(*member_id);
            let Some((candidate, _plan)) = self.resolve_target_id(*member_id) else {
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
        let selected_tag = self
            .target_tag(selected)
            .unwrap_or_else(|| "<unknown>".to_owned());
        let previous_tag = previous.and_then(|target| self.target_tag(target));

        let latency_ms = best
            .as_ref()
            .and_then(|probe| (probe.outbound_id == selected).then_some(probe.latency_ms));

        let probe_members: Vec<PolicyProbeMember> = member_states
            .iter()
            .map(|state| {
                let tag = self
                    .target_tag(state.member_id)
                    .unwrap_or_else(|| "<unknown>".to_owned());
                PolicyProbeMember {
                    target_tag: tag,
                    healthy: state.healthy,
                    latency_ms: state.latency_ms,
                    error: state.last_error.clone(),
                }
            })
            .collect();

        self.update_urltest_state(group_id, selected, latency_ms, member_states);

        self.engine().push_policy_probe_completed(
            group_tag,
            PolicyProbeCompletedPayload {
                policy_tag: group_tag.to_owned(),
                selected: Some(selected_tag.clone()),
                members: probe_members,
            },
        );

        log_urltest_group_target_changed(
            group_tag,
            previous_tag.as_deref(),
            &selected_tag,
            latency_ms,
        );

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
            ResolvedOutbound::Relay { .. } => Err(EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "relay chain cannot be used as a urltest member",
            ))),
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

    /// Probe a single outbound **through the proxy stack** (full TLS + protocol
    /// handshake, then an HTTP HEAD to `url`, time to first byte) and return
    /// the latency in milliseconds.
    ///
    /// This is the synchronous, single-node counterpart to the async group
    /// probe (`policies.probe`), and unlike the engine's direct-TCP
    /// `probe_target` it measures the real end-to-end proxy path. Used by the
    /// `diagnostics.probe_outbound` command for GUI "tap one node to test".
    pub async fn probe_outbound_single(
        &self,
        target_tag: &str,
        url: &str,
    ) -> Result<u64, EngineError> {
        let probe =
            UrlTestProbe::parse(url).map_err(|message| EngineError::InvalidUrlTestGroup {
                tag: target_tag.to_owned(),
                message,
            })?;
        let plan = self.engine().plan();
        let Some(target_id) = plan.target_id(target_tag) else {
            return Err(EngineError::SelectorGroupNotFound {
                tag: target_tag.to_owned(),
            });
        };
        let Some((candidate, _plan)) = self.resolve_target_id(target_id) else {
            return Err(EngineError::SelectorGroupNotFound {
                tag: target_tag.to_owned(),
            });
        };
        self.probe_outbound(candidate, &probe).await
    }

    async fn probe_leaf_outbound(
        &self,
        candidate: ResolvedLeafOutbound<'_>,
        probe: &UrlTestProbe,
    ) -> Result<u64, EngineError> {
        const URLTEST_PROBE_TIMEOUT: Duration = Duration::from_secs(5);

        timeout(URLTEST_PROBE_TIMEOUT, async {
            let started_at = Instant::now();

            // Build a dummy session for the probe target — the outbound
            // establishment pipeline will connect through the candidate.
            let probe_session = Session::new(
                0,
                Address::Domain(probe.host.clone()),
                probe.port,
                Network::Tcp,
                ProtocolType::Unknown,
            );

            // Use the existing TCP outbound establishment pipeline which
            // handles ALL protocol types generically (Direct, Socks5,
            // Vless, Hysteria2, Shadowsocks, Trojan, Vmess, Mieru, etc.).
            let outbound = self
                .dispatch_tcp_candidate(&probe_session, candidate)
                .await
                .map_err(|f| f.error)?;
            let result = extract_tcp_stream(outbound)?;
            let mut socket = result.upstream;

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
