//! Direct inbound — fixed-target forwarder.
//!
//! Listens on a port, accepts raw TCP connections with no protocol
//! handshake, and forwards all traffic to a configured outbound
//! (node or group).  Target address comes from the inbound config.

use std::io;

use tokio::select;
use tokio::sync::watch;
use tokio::task::JoinSet;
use tracing::{error, info};
use zero_core::{Address, Network, ProtocolType, Session};
use zero_engine::EngineError;

use crate::logging::log_listener_connection_error;
use crate::runtime::bind_listener;
use crate::runtime::inbound_protocol::apply_kernel_rate_limits;
use crate::runtime::Proxy;
use crate::transport::{relay_bidirectional_metered_throttled, TcpRelayStream};

impl Proxy {
    pub(crate) async fn run_direct_listener(
        &self,
        inbound: zero_config::InboundConfig,
        mut shutdown: watch::Receiver<bool>,
    ) -> Result<(), EngineError> {
        let (outbound_tag, target, port) = match &inbound.protocol {
            zero_config::InboundProtocolConfig::Direct {
                outbound,
                target,
                port,
            } => (outbound.clone(), target.clone(), *port),
            _ => {
                return Err(EngineError::Io(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "direct listener requires direct config",
                )))
            }
        };

        let listener = bind_listener(&inbound).await?;
        let local_addr = listener.local_addr()?;
        let mut connections = JoinSet::new();

        info!(
            inbound_tag = %inbound.tag,
            protocol = "direct",
            outbound = %outbound_tag,
            target = ?target,
            port = ?port,
            listen = %local_addr,
            "inbound listener ready"
        );

        loop {
            select! {
                changed = shutdown.changed() => {
                    match changed {
                        Ok(()) if *shutdown.borrow() => break,
                        Ok(()) => {}
                        Err(_) => break,
                    }
                }
                accept_result = listener.accept() => {
                    match accept_result {
                        Ok((stream, remote_addr)) => {
                            let engine = self.clone();
                            let tag = inbound.tag.clone();
                            let outbound = outbound_tag.clone();
                            let target = target.clone();
                            let port = port;
                            connections.spawn(async move {
                                if let Err(error) = engine.serve_direct_connection(
                                    stream, &tag, &outbound, target.as_deref(), port,
                                ).await {
                                    log_listener_connection_error(
                                        "direct", &tag, &remote_addr, &error,
                                    );
                                }
                            });
                        }
                        Err(e) => {
                            error!(error = %e, "direct: accept error");
                        }
                    }
                }
                result = connections.join_next(), if !connections.is_empty() => {
                    if let Some(Err(error)) = result {
                        if !error.is_cancelled() {
                            error!(error = %error, "direct connection task panicked");
                        }
                    }
                }
            }
        }

        connections.abort_all();
        while let Some(result) = connections.join_next().await {
            if let Err(error) = result {
                if !error.is_cancelled() {
                    error!(error = %error, "direct connection task panicked during shutdown");
                }
            }
        }

        info!(inbound_tag = %inbound.tag, protocol = "direct", listen = %local_addr, "inbound listener stopped");
        Ok(())
    }

    /// Handle a single direct inbound connection.
    async fn serve_direct_connection(
        &self,
        client: impl Into<TcpRelayStream>,
        inbound_tag: &str,
        outbound_tag: &str,
        target: Option<&str>,
        target_port: Option<u16>,
    ) -> Result<(), EngineError> {
        let client: TcpRelayStream = client.into();

        // Build session target from config.
        let address = match target {
            Some(domain) if domain.parse::<std::net::Ipv4Addr>().is_ok() => {
                Address::Ipv4(domain.parse::<std::net::Ipv4Addr>().unwrap().octets())
            }
            Some(ip_str) if ip_str.parse::<std::net::Ipv6Addr>().is_ok() => {
                Address::Ipv6(ip_str.parse::<std::net::Ipv6Addr>().unwrap().octets())
            }
            Some(domain) => Address::Domain(domain.to_owned()),
            None => {
                return Err(EngineError::Io(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "direct inbound requires a target address in config",
                )))
            }
        };
        let port = target_port.unwrap_or(443);

        let mut session = Session::new(0, address, port, Network::Tcp, ProtocolType::Unknown);

        // ── Kernel: rate policy ──
        apply_kernel_rate_limits(self, &mut session, inbound_tag);

        self.prepare_session(&mut session, inbound_tag, None);
        let up_bps = session.up_bps;
        let down_bps = session.down_bps;

        // ── Kernel: session tracking ──
        let mut handle = self.track_session(session.id);

        // Resolve the configured outbound by tag (handles groups).
        let target_id = self
            .engine()
            .plan()
            .target_id(outbound_tag)
            .ok_or_else(|| EngineError::MissingRouteTarget {
                tag: outbound_tag.to_owned(),
            })?;
        let (resolved, plan) =
            self.resolve_target_id(target_id)
                .ok_or_else(|| EngineError::InvalidPlan {
                    message: format!(
                        "direct inbound '{}': outbound '{}' could not be resolved",
                        inbound_tag, outbound_tag,
                    ),
                })?;

        // ── Kernel: circuit breaker check ──
        // Hmm, this doesn't go through establish_tcp_candidate which has
        // the health check.  We call establish_tcp_outbound directly,
        // so the check is there already.

        let upstream = match self
            .establish_tcp_outbound(&session, (resolved, Some(plan)))
            .await
        {
            Ok(result) => result,
            Err(failure) => {
                handle.finish(zero_engine::SessionOutcome::Failed);
                return Err(EngineError::Io(io::Error::new(
                    io::ErrorKind::Other,
                    failure.error,
                )));
            }
        };

        let upstream = match upstream {
            crate::transport::EstablishedTcpOutbound::Direct { upstream, .. }
            | crate::transport::EstablishedTcpOutbound::Socks5 { upstream, .. }
            | crate::transport::EstablishedTcpOutbound::Vless { upstream, .. }
            | crate::transport::EstablishedTcpOutbound::Hysteria2 { upstream, .. }
            | crate::transport::EstablishedTcpOutbound::Shadowsocks { upstream, .. }
            | crate::transport::EstablishedTcpOutbound::Trojan { upstream, .. }
            | crate::transport::EstablishedTcpOutbound::Vmess { upstream, .. }
            | crate::transport::EstablishedTcpOutbound::Relay { upstream } => upstream,
            crate::transport::EstablishedTcpOutbound::Block { .. } => {
                handle.finish(zero_engine::SessionOutcome::Blocked);
                return Err(EngineError::Io(io::Error::new(
                    io::ErrorKind::ConnectionRefused,
                    "direct inbound: outbound resolved to block",
                )));
            }
        };

        let relay_result = relay_bidirectional_metered_throttled(
            client,
            upstream,
            |_| {},
            |_| {},
            up_bps,
            down_bps,
        )
        .await;

        match relay_result {
            Ok(_) => {
                handle.finish(zero_engine::SessionOutcome::DirectRelayed);
                Ok(())
            }
            Err(e) => {
                handle.finish(zero_engine::SessionOutcome::Failed);
                Err(EngineError::Io(e))
            }
        }
    }
}
