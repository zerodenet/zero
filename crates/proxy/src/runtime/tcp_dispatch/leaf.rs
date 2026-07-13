use std::io;
use std::sync::Arc;

use zero_core::Session;
use zero_engine::{EngineError, EnginePlan, ResolvedLeafOutbound, ResolvedOutbound};

use crate::runtime::path::{OutboundEndpoint, TcpPathCategory};
use crate::runtime::Proxy;
use crate::transport::{
    extract_tcp_stream, EstablishedTcpOutbound, TcpOutboundFailure, TcpRouteResult,
};

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
        let chained_tag: Option<String> = match path_category {
            TcpPathCategory::Direct | TcpPathCategory::Block => None,
            #[cfg(any(feature = "socks5", feature = "vless", feature = "trojan"))]
            TcpPathCategory::Tunnel => runtime.health_tag.map(ToOwned::to_owned),
            #[cfg(any(feature = "shadowsocks", feature = "vmess", feature = "mieru"))]
            TcpPathCategory::Session => runtime.health_tag.map(ToOwned::to_owned),
            #[cfg(feature = "hysteria2")]
            TcpPathCategory::TransportSession => runtime.health_tag.map(ToOwned::to_owned),
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
        // Direct and every proxy protocol go through the adapter registry;
        // adding a protocol = register an adapter, zero changes here.
        let result = if matches!(path_category, TcpPathCategory::Block) {
            Ok(EstablishedTcpOutbound::block(
                runtime.kernel_tag.unwrap_or("block"),
            ))
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
