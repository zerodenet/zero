use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};

use crate::runtime::path::OutboundEndpoint;
use crate::runtime::Proxy;
use crate::transport::{EstablishedTcpOutbound, RelayCarrier, TcpOutboundFailure, TcpRelayStream};

impl Proxy {
    pub(super) async fn dispatch_tcp_relay_chain<'a>(
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
