use zero_engine::EngineError;

use super::super::ProtocolInventory;
use crate::protocol_registry::{ClaimedOutboundLeaf, UdpAdapterContext};
use crate::runtime::udp_dispatch::operation::PreparedUdpFlowOperation;
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::runtime::udp_flow::packet_path::{PacketPathFlowBinding, UdpPacketRef};
use crate::runtime::udp_flow::packet_path_chain::PacketPathStartRequest;
use crate::transport::RelayCarrier;

enum PreparedUdpRelayChain<'a> {
    Operation(Box<dyn PreparedUdpFlowOperation + 'a>),
}

impl PreparedUdpRelayChain<'_> {
    async fn execute(
        self,
        dispatch: &mut UdpDispatch,
        ctx: UdpAdapterContext<'_>,
        session: &zero_core::Session,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        match self {
            PreparedUdpRelayChain::Operation(operation) => {
                operation.execute(dispatch, ctx, session, payload).await
            }
        }
    }
}

impl ProtocolInventory {
    /// Prepare the packet-path carrier/datagram pair and lazy carrier builder
    /// for a two-hop UDP relay chain.
    pub(crate) fn prepare_udp_packet_path_pair<'a>(
        &self,
        session_id: u64,
        carrier_leaf: &'a zero_engine::ResolvedLeafOutbound<'a>,
        datagram_leaf: &'a zero_engine::ResolvedLeafOutbound<'a>,
        packet: UdpPacketRef<'a>,
    ) -> Option<(PacketPathFlowBinding, PacketPathStartRequest<'a>)> {
        let carrier_operation = self
            .claim_outbound_leaf(carrier_leaf)
            .ok()?
            .prepare_udp_packet_path(carrier_leaf)?;
        let datagram_operation = self
            .claim_outbound_leaf(datagram_leaf)
            .ok()?
            .prepare_udp_packet_path(datagram_leaf)?;

        super::packet_path::build_udp_packet_path_pair(
            session_id,
            carrier_operation,
            datagram_operation,
            packet,
        )
    }

    /// Whether the UDP relay final hop needs the VLESS two-stream path.
    pub(crate) fn udp_relay_needs_two_streams(
        &self,
        ctx: UdpAdapterContext<'_>,
        leaf: &zero_engine::ResolvedLeafOutbound<'_>,
    ) -> Result<bool, EngineError> {
        let claimed = self.claim_outbound_leaf(leaf)?;
        Ok(claimed.udp_relay_needs_two_streams(ctx.source_dir()))
    }

    /// Start a two-stream UDP relay path through the final hop adapter.
    #[cfg(test)]
    pub(crate) async fn start_udp_relay_two_stream(
        &self,
        dispatch: &mut crate::runtime::udp_dispatch::UdpDispatch,
        ctx: UdpAdapterContext<'_>,
        session: &zero_core::Session,
        chain: Vec<zero_engine::ResolvedLeafOutbound<'_>>,
        payload: &[u8],
    ) -> Result<
        crate::runtime::udp_dispatch::FlowStartResult,
        crate::runtime::udp_dispatch::FlowFailure,
    > {
        let final_hop = chain.last().expect("relay chain has at least 2 hops");
        let claimed = self.claim_outbound_leaf(final_hop).map_err(|error| {
            crate::runtime::udp_dispatch::FlowFailure {
                stage: "find_outbound_leaf",
                error,
                upstream: None,
            }
        })?;
        let (post_stream, _post_peer) = tokio::io::duplex(64);
        let (get_stream, _get_peer) = tokio::io::duplex(64);
        let operation = self.prepare_udp_relay_two_stream_operation(
            &claimed,
            ctx.clone(),
            RelayCarrier {
                stream: crate::transport::TcpRelayStream::new(post_stream),
                server: "fake-relay-post.test".to_owned(),
                port: 9443,
            },
            RelayCarrier {
                stream: crate::transport::TcpRelayStream::new(get_stream),
                server: "fake-relay-get.test".to_owned(),
                port: 9444,
            },
        )?;
        operation.execute(dispatch, ctx, session, payload).await
    }

    /// Start a single-stream UDP relay final hop through the final hop adapter.
    #[cfg(test)]
    pub(crate) async fn start_udp_relay_final_hop(
        &self,
        dispatch: &mut UdpDispatch,
        ctx: UdpAdapterContext<'_>,
        session: &zero_core::Session,
        carrier: crate::transport::RelayCarrier,
        leaf: &zero_engine::ResolvedLeafOutbound<'_>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        let operation =
            self.prepare_udp_relay_final_hop_operation(ctx.clone(), carrier, leaf.clone())?;
        operation.execute(dispatch, ctx, session, payload).await
    }

    pub(crate) async fn start_udp_relay_chain(
        &self,
        dispatch: &mut UdpDispatch,
        ctx: UdpAdapterContext<'_>,
        session: &zero_core::Session,
        chain: Vec<zero_engine::ResolvedLeafOutbound<'_>>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        self.validate_udp_relay_chain(ctx.clone(), &chain)?;

        if chain.len() == 2 {
            let carrier_leaf = &chain[0];
            let datagram_leaf = &chain[1];
            if let Some((flow_binding, request)) = self.prepare_udp_packet_path_pair(
                session.id,
                carrier_leaf,
                datagram_leaf,
                UdpPacketRef {
                    target: &session.target,
                    port: session.port,
                    payload,
                },
            ) {
                let sent = dispatch.send_packet_path_chain(ctx, request).await?;
                return Ok(FlowStartResult::Flow {
                    outbound: Box::new(UdpDispatch::datagram_chain_flow_outbound(flow_binding)),
                    tx_bytes: sent as u64,
                });
            }
        }

        let prepared = self.prepare_udp_relay_chain(ctx.clone(), chain).await?;
        prepared.execute(dispatch, ctx, session, payload).await
    }
}

impl ProtocolInventory {
    async fn prepare_udp_relay_chain<'a>(
        &self,
        ctx: UdpAdapterContext<'a>,
        chain: Vec<zero_engine::ResolvedLeafOutbound<'a>>,
    ) -> Result<PreparedUdpRelayChain<'a>, FlowFailure> {
        let final_hop = chain.last().expect("relay chain has at least 2 hops");
        if self
            .udp_relay_needs_two_streams(ctx.clone(), final_hop)
            .map_err(|error| FlowFailure {
                stage: "find_outbound_leaf",
                error,
                upstream: None,
            })?
        {
            let claimed = self
                .claim_outbound_leaf(final_hop)
                .map_err(|error| FlowFailure {
                    stage: "find_outbound_leaf",
                    error,
                    upstream: None,
                })?;
            let services = ctx.runtime_services();
            let post_prepared = services
                .prepare_tcp_relay_chain(&chain)
                .map_err(flow_failure_from_tcp_outbound)?;
            let post_carrier = services
                .dispatch_prepared_tcp_relay_carrier(post_prepared)
                .await
                .map_err(flow_failure_from_tcp_outbound)?;
            let get_prepared = services
                .prepare_tcp_relay_chain(&chain)
                .map_err(flow_failure_from_tcp_outbound)?;
            let get_carrier = services
                .dispatch_prepared_tcp_relay_carrier(get_prepared)
                .await
                .map_err(flow_failure_from_tcp_outbound)?;
            let operation = self.prepare_udp_relay_two_stream_operation(
                &claimed,
                ctx.clone(),
                post_carrier,
                get_carrier,
            )?;
            return Ok(PreparedUdpRelayChain::Operation(operation));
        }

        let final_hop = chain
            .last()
            .cloned()
            .expect("relay chain has at least 2 hops");
        let services = ctx.runtime_services();
        let prepared_prefix = services
            .prepare_tcp_relay_chain(&chain)
            .map_err(|failure| FlowFailure {
                stage: failure.stage,
                error: failure.error,
                upstream: failure.upstream_endpoint,
            })?;
        let carrier = services
            .dispatch_prepared_tcp_relay_carrier(prepared_prefix)
            .await
            .map_err(|failure| FlowFailure {
                stage: failure.stage,
                error: failure.error,
                upstream: failure.upstream_endpoint,
            })?;
        let operation =
            self.prepare_udp_relay_final_hop_operation(ctx.clone(), carrier, final_hop)?;
        Ok(PreparedUdpRelayChain::Operation(operation))
    }

    fn validate_udp_relay_chain(
        &self,
        ctx: UdpAdapterContext<'_>,
        chain: &[zero_engine::ResolvedLeafOutbound<'_>],
    ) -> Result<(), FlowFailure> {
        for leaf in chain {
            let runtime = self
                .claim_outbound_leaf(leaf)
                .map_err(|error| FlowFailure {
                    stage: "outbound_leaf_runtime",
                    error,
                    upstream: None,
                })?
                .runtime;
            if !ctx.udp_enabled_for_outbound(runtime.udp_policy_tag) {
                return Err(FlowFailure {
                    stage: "udp_policy",
                    error: EngineError::Io(std::io::Error::other(
                        "udp disabled for relay chain outbound",
                    )),
                    upstream: runtime
                        .endpoint
                        .map(|endpoint| (endpoint.server.to_owned(), endpoint.port)),
                });
            }
        }
        Ok(())
    }

    fn prepare_udp_relay_two_stream_operation<'a>(
        &self,
        claimed: &ClaimedOutboundLeaf<'a>,
        ctx: UdpAdapterContext<'a>,
        post_carrier: RelayCarrier,
        get_carrier: RelayCarrier,
    ) -> Result<Box<dyn PreparedUdpFlowOperation + 'a>, FlowFailure> {
        claimed.prepare_owned_udp_relay_two_stream(post_carrier, get_carrier, ctx.source_dir())
    }

    fn prepare_udp_relay_final_hop_operation<'a>(
        &self,
        ctx: UdpAdapterContext<'a>,
        carrier: RelayCarrier,
        leaf: zero_engine::ResolvedLeafOutbound<'a>,
    ) -> Result<Box<dyn PreparedUdpFlowOperation + 'a>, FlowFailure> {
        let claimed = self
            .claim_outbound_leaf(&leaf)
            .map_err(|error| FlowFailure {
                stage: "find_outbound_leaf",
                error,
                upstream: None,
            })?;
        let _ = leaf;
        claimed.prepare_owned_udp_relay_final_hop(carrier, ctx.source_dir())
    }
}

fn flow_failure_from_tcp_outbound(failure: crate::transport::TcpOutboundFailure) -> FlowFailure {
    FlowFailure {
        stage: failure.stage,
        error: failure.error,
        upstream: failure.upstream_endpoint,
    }
}
