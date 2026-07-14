use std::io;

use zero_core::Session;
use zero_engine::EngineError;
use zero_engine::{ResolvedLeafOutbound, ResolvedOutbound};

use super::ProtocolInventory;
use crate::protocol_registry::{OutboundAdapterContext, TcpOutboundCapability};
use crate::runtime::path::{OutboundEndpoint, TcpPathCategory};
use crate::runtime::Proxy;
use crate::transport::{EstablishedTcpOutbound, RelayCarrier, TcpOutboundFailure, TcpRelayStream};

impl ProtocolInventory {
    /// Establish a TCP outbound through the adapter that owns `leaf`.
    pub(crate) async fn connect_tcp_leaf(
        &self,
        proxy: &Proxy,
        session: &zero_core::Session,
        leaf: &zero_engine::ResolvedLeafOutbound<'_>,
    ) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
        let adapter =
            self.registry
                .find_outbound_leaf(leaf)
                .map_err(|error| TcpOutboundFailure {
                    stage: "find_outbound_leaf",
                    error,
                    upstream_endpoint: None,
                })?;
        let operation = TcpOutboundCapability::prepare_tcp_connect(
            adapter.as_ref(),
            leaf,
            proxy.config.source_dir(),
        )?;
        operation
            .execute(OutboundAdapterContext::new(proxy), session)
            .await
    }

    /// Apply one relay-chain TCP hop through the adapter that owns `leaf`.
    pub(crate) async fn apply_tcp_relay_hop(
        &self,
        proxy: &Proxy,
        stream: TcpRelayStream,
        session: &zero_core::Session,
        leaf: &zero_engine::ResolvedLeafOutbound<'_>,
    ) -> Result<TcpRelayStream, EngineError> {
        let adapter = self.registry.find_outbound_leaf(leaf)?;
        let operation = TcpOutboundCapability::prepare_tcp_relay_hop(
            adapter.as_ref(),
            leaf,
            proxy.config.source_dir(),
        )?;
        operation
            .execute(OutboundAdapterContext::new(proxy), stream, session)
            .await
    }
}

impl Proxy {
    pub(crate) async fn dispatch_tcp_outbound(
        &self,
        session: &Session,
        resolved: ResolvedOutbound<'static>,
    ) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
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
            if let Err(error) = self.check_outbound_health(tag) {
                return Err(TcpOutboundFailure {
                    stage: "health_check",
                    error,
                    upstream_endpoint: None,
                });
            }
        }

        let result = if matches!(path_category, TcpPathCategory::Block) {
            Ok(EstablishedTcpOutbound::block(
                runtime.kernel_tag.unwrap_or("block"),
            ))
        } else {
            self.protocols
                .connect_tcp_leaf(self, session, &candidate)
                .await
        };

        if let Some(tag) = chained_tag.as_deref() {
            match &result {
                Ok(_) => self.record_outbound_success(tag),
                Err(_) => self.record_outbound_failure(tag),
            }
        }

        result
    }

    pub(crate) async fn dispatch_tcp_relay_chain<'a>(
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

        Ok(EstablishedTcpOutbound::relay(stream))
    }

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
        let mut stream = outbound
            .into_relay_stream()
            .map_err(|error| TcpOutboundFailure {
                stage: "relay_first_hop",
                error,
                upstream_endpoint: None,
            })?;

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

        let endpoint = self.outbound_endpoint(&current_hop)?;
        Ok((
            RelayCarrier {
                stream,
                server: endpoint.server.to_owned(),
                port: endpoint.port,
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
                error: EngineError::Io(io::Error::new(
                    io::ErrorKind::InvalidInput,
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
