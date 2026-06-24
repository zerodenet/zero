//! TCP connection dispatch: routing pipeline and outbound orchestration.
//!
//! Moved from transport/tcp_outbound.rs; these methods are runtime orchestration,
//! not transport I/O.

use std::io;
use std::sync::Arc;

use zero_core::Session;

use crate::runtime::orchestration::{OutboundEndpoint, TcpPathCategory};
use crate::runtime::Proxy;
use crate::transport::{
    extract_tcp_stream, EstablishedTcpOutbound, RelayCarrier, TcpOutboundFailure, TcpRelayStream,
    TcpRouteResult,
};
use zero_engine::{EngineError, EnginePlan};
use zero_engine::{ResolvedLeafOutbound, ResolvedOutbound};

impl Proxy {
    /// Execute the unified routing and outbound establishment pipeline.
    ///
    /// Caller MUST call `prepare_session` before this to assign a session ID.
    pub(crate) async fn dispatch_tcp(
        &self,
        session: &mut Session,
    ) -> Result<TcpRouteResult, EngineError> {
        self.resolve_fake_ip_target(session).await;
        let action = self.route_decision(session);
        let (resolved, _plan) = self.resolve_outbound(&action)?;
        let outbound = self
            .dispatch_tcp_outbound(session, (resolved, _plan))
            .await
            .map_err(|f| EngineError::Io(io::Error::other(f.error)))?;
        let mut result = extract_tcp_stream(outbound)?;
        result.route_action = action;
        Ok(result)
    }

    async fn dispatch_tcp_outbound(
        &self,
        session: &Session,
        resolved: (ResolvedOutbound<'static>, Option<Arc<EnginePlan>>),
    ) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
        let (resolved, _plan) = resolved;
        match resolved {
            ResolvedOutbound::Relay { chain } => {
                self.dispatch_tcp_relay_chain(session, chain).await
            }
            ResolvedOutbound::Single(candidate) => {
                self.dispatch_tcp_candidate(session, candidate).await
            }
            ResolvedOutbound::Fallback { candidates } => {
                let mut last_failure = None;

                for candidate in candidates {
                    match self.dispatch_tcp_candidate(session, candidate).await {
                        Ok(outbound) => return Ok(outbound),
                        Err(failure) => last_failure = Some(failure),
                    }
                }

                Err(last_failure
                    .expect("validated fallback groups always have at least one candidate"))
            }
        }
    }

    pub(crate) async fn dispatch_tcp_candidate(
        &self,
        session: &Session,
        candidate: ResolvedLeafOutbound<'_>,
    ) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
        // Kernel primitive: circuit breaker.
        // Check health before connecting (skip for Direct / Block).
        let runtime = self
            .protocols
            .outbound_leaf_runtime(&candidate)
            .map_err(|error| TcpOutboundFailure {
                stage: "outbound_leaf_runtime",
                error,
                upstream_endpoint: None,
            })?;
        let path_category = runtime.tcp_path;
        let chained_tag = match path_category {
            TcpPathCategory::Direct | TcpPathCategory::Block => None,
            TcpPathCategory::Tunnel
            | TcpPathCategory::Session
            | TcpPathCategory::TransportSession => runtime.health_tag.map(ToOwned::to_owned),
        };
        if let Some(tag) = chained_tag.as_deref() {
            if let Err(e) = self.check_outbound_health(tag) {
                return Err(TcpOutboundFailure {
                    stage: "health_check",
                    error: e,
                    upstream_endpoint: None,
                });
            }
        }

        // Block is kernel-level (no adapter owns it): reject immediately.
        // Direct and every proxy protocol go through the adapter registry —
        // adding a protocol = register an adapter, zero changes here.
        let result = if matches!(path_category, TcpPathCategory::Block) {
            Ok(EstablishedTcpOutbound::Block {
                tag: runtime.kernel_tag.unwrap_or("block").to_string(),
            })
        } else {
            self.protocols
                .connect_tcp_leaf(self, session, &candidate)
                .await
        };

        // Record health after connection attempt.
        if let Some(tag) = chained_tag.as_deref() {
            match &result {
                Ok(_) => self.record_outbound_success(tag),
                Err(_) => self.record_outbound_failure(tag),
            }
        }

        result
    }

    async fn dispatch_tcp_relay_chain<'a>(
        &self,
        session: &Session,
        chain: Vec<ResolvedLeafOutbound<'a>>,
    ) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
        let (carrier, final_hop) = self.dispatch_tcp_relay_prefix(chain).await?;

        let stream = apply_hop_protocol(self, carrier.stream, &final_hop, session)
            .await
            .map_err(|error| TcpOutboundFailure {
                stage: "relay_last",
                error,
                upstream_endpoint: None,
            })?;

        Ok(EstablishedTcpOutbound::Relay { upstream: stream })
    }

    /// Establish all relay hops before the final protocol hop.
    ///
    /// The returned stream is connected to the final hop server through the
    /// preceding relay hops. The caller is responsible for running the final
    /// hop protocol handshake on that stream.
    pub(crate) async fn dispatch_tcp_relay_prefix<'a>(
        &self,
        chain: Vec<ResolvedLeafOutbound<'a>>,
    ) -> Result<(RelayCarrier, ResolvedLeafOutbound<'a>), TcpOutboundFailure> {
        let mut hops = chain.into_iter();
        let first = hops.next().expect("relay chain must have at least 2 hops");
        let second = hops.next().expect("relay chain must have at least 2 hops");

        let second_endpoint = self.outbound_endpoint(&second)?;
        let mut session_for_next = relay_next_session(second_endpoint);

        let outbound = self
            .dispatch_tcp_candidate(&session_for_next, first)
            .await?;
        let mut stream = match outbound {
            EstablishedTcpOutbound::Direct { upstream, .. }
            | EstablishedTcpOutbound::Socks5 { upstream, .. }
            | EstablishedTcpOutbound::Vless { upstream, .. }
            | EstablishedTcpOutbound::Hysteria2 { upstream, .. }
            | EstablishedTcpOutbound::Shadowsocks { upstream, .. }
            | EstablishedTcpOutbound::Trojan { upstream, .. }
            | EstablishedTcpOutbound::Vmess { upstream, .. }
            | EstablishedTcpOutbound::Mieru { upstream, .. }
            | EstablishedTcpOutbound::Relay { upstream } => upstream,
            EstablishedTcpOutbound::Block { .. } => {
                return Err(TcpOutboundFailure {
                    stage: "relay_first_hop",
                    error: EngineError::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "first relay hop resolved to block",
                    )),
                    upstream_endpoint: None,
                })
            }
        };

        let mut current_hop = second;
        for next_hop in hops {
            session_for_next = relay_next_session(self.outbound_endpoint(&next_hop)?);
            stream = apply_hop_protocol(self, stream, &current_hop, &session_for_next)
                .await
                .map_err(|error| TcpOutboundFailure {
                    stage: "relay_hop",
                    error,
                    upstream_endpoint: None,
                })?;
            current_hop = next_hop;
        }

        let ep = self.outbound_endpoint(&current_hop)?;
        Ok((
            RelayCarrier {
                stream,
                server: ep.server.to_owned(),
                port: ep.port,
            },
            current_hop,
        ))
    }

    pub(crate) fn outbound_endpoint<'a>(
        &self,
        leaf: &ResolvedLeafOutbound<'a>,
    ) -> Result<OutboundEndpoint<'a>, TcpOutboundFailure> {
        self.protocols
            .outbound_leaf_runtime(leaf)
            .map_err(|error| TcpOutboundFailure {
                stage: "outbound_leaf_runtime",
                error,
                upstream_endpoint: None,
            })?
            .endpoint
            .ok_or_else(|| TcpOutboundFailure {
                stage: "outbound_leaf_endpoint",
                error: EngineError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "relay hop resolved without upstream endpoint",
                )),
                upstream_endpoint: None,
            })
    }
}

fn relay_next_session(endpoint: OutboundEndpoint<'_>) -> Session {
    Session::new(
        0,
        endpoint.address(),
        endpoint.port,
        zero_core::Network::Tcp,
        zero_core::ProtocolType::Unknown,
    )
}

/// Apply a single hop's protocol request to an existing stream.
///
/// Single dispatch point: delegates to ProtocolInventory, which resolves the
/// hop to its registered adapter. Adding a protocol = register an adapter;
/// this function never matches on the protocol enum.
async fn apply_hop_protocol(
    proxy: &Proxy,
    stream: TcpRelayStream,
    hop: &ResolvedLeafOutbound<'_>,
    session: &Session,
) -> Result<TcpRelayStream, EngineError> {
    proxy
        .protocols
        .apply_tcp_relay_hop(proxy, stream, session, hop)
        .await
}
