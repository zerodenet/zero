use zero_engine::EngineError;

use super::super::ClaimedInventoryLeaf;
use super::super::{ClaimedRelayChain, ProtocolInventory};
use crate::protocol_registry::{ClaimedOutboundLeaf, OutboundAdapterContext, UdpAdapterContext};
use crate::runtime::udp_dispatch::operation::PreparedUdpFlowOperation;
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::runtime::udp_flow::packet_path::{PacketPathFlowBinding, UdpPacketRef};
use crate::runtime::udp_flow::packet_path_chain::PacketPathStartRequest;
use crate::transport::RelayCarrier;

pub(super) enum PreparedUdpRelayChain<'a> {
    PacketPath {
        flow_binding: PacketPathFlowBinding,
        request: Box<PacketPathStartRequest<'a>>,
    },
    Operation(Box<dyn PreparedUdpFlowOperation + 'a>),
}

impl PreparedUdpRelayChain<'_> {
    pub(super) async fn execute(
        self,
        dispatch: &mut UdpDispatch,
        ctx: UdpAdapterContext<'_>,
        session: &zero_core::Session,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        match self {
            PreparedUdpRelayChain::PacketPath {
                flow_binding,
                request,
            } => {
                let sent = dispatch.send_packet_path_chain(ctx, *request).await?;
                Ok(FlowStartResult::Flow {
                    outbound: Box::new(UdpDispatch::datagram_chain_flow_outbound(flow_binding)),
                    tx_bytes: sent as u64,
                })
            }
            PreparedUdpRelayChain::Operation(operation) => {
                operation.execute(dispatch, ctx, session, payload).await
            }
        }
    }
}

impl ProtocolInventory {
    pub(in crate::inventory) fn prepare_claimed_udp_packet_path_pair<'a>(
        &self,
        session_id: u64,
        carrier_leaf: &ClaimedInventoryLeaf<'a>,
        datagram_leaf: &ClaimedInventoryLeaf<'a>,
        packet: UdpPacketRef<'a>,
    ) -> Option<(PacketPathFlowBinding, PacketPathStartRequest<'a>)> {
        let carrier_operation = carrier_leaf.prepare_udp_packet_path()?;
        let datagram_operation = datagram_leaf.prepare_udp_packet_path()?;

        super::packet_path::build_udp_packet_path_pair(
            session_id,
            carrier_operation,
            datagram_operation,
            packet,
        )
    }
    pub(super) async fn prepare_claimed_udp_relay_chain<'a>(
        &self,
        ctx: UdpAdapterContext<'a>,
        session: &'a zero_core::Session,
        claimed_chain: &ClaimedRelayChain<'a>,
        payload: &'a [u8],
    ) -> Result<PreparedUdpRelayChain<'a>, FlowFailure> {
        self.validate_udp_relay_chain(ctx.clone(), claimed_chain)?;
        if claimed_chain.len() == 2 {
            if let Some((flow_binding, request)) = self.prepare_claimed_udp_packet_path_pair(
                session.id,
                claimed_chain.first(),
                claimed_chain.final_hop(),
                UdpPacketRef {
                    target: &session.target,
                    port: session.port,
                    payload,
                },
            ) {
                return Ok(PreparedUdpRelayChain::PacketPath {
                    flow_binding,
                    request: Box::new(request),
                });
            }
        }

        let needs_two_streams = claimed_chain
            .final_hop()
            .udp_relay_needs_two_streams(ctx.source_dir());
        let final_hop = claimed_chain.final_hop().clone().into_claimed();

        if needs_two_streams {
            let services = ctx.runtime_services();
            let outbound_ctx = OutboundAdapterContext::new(ctx.source_dir());
            let post_prepared = self
                .prepare_claimed_tcp_relay_chain(outbound_ctx.clone(), claimed_chain)
                .map_err(flow_failure_from_tcp_outbound)?;
            let post_carrier = services
                .dispatch_prepared_tcp_relay_carrier(post_prepared)
                .await
                .map_err(flow_failure_from_tcp_outbound)?;
            let get_prepared = self
                .prepare_claimed_tcp_relay_chain(outbound_ctx, claimed_chain)
                .map_err(flow_failure_from_tcp_outbound)?;
            let get_carrier = services
                .dispatch_prepared_tcp_relay_carrier(get_prepared)
                .await
                .map_err(flow_failure_from_tcp_outbound)?;
            let operation = self.prepare_udp_relay_two_stream_operation(
                final_hop,
                ctx.clone(),
                post_carrier,
                get_carrier,
            )?;
            return Ok(PreparedUdpRelayChain::Operation(operation));
        }

        let services = ctx.runtime_services();
        let prepared_prefix = self
            .prepare_claimed_tcp_relay_chain(
                OutboundAdapterContext::new(ctx.source_dir()),
                claimed_chain,
            )
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
            self.prepare_udp_relay_final_hop_operation(final_hop, ctx.clone(), carrier)?;
        Ok(PreparedUdpRelayChain::Operation(operation))
    }

    fn validate_udp_relay_chain(
        &self,
        ctx: UdpAdapterContext<'_>,
        chain: &ClaimedRelayChain<'_>,
    ) -> Result<(), FlowFailure> {
        for leaf in chain.leaves() {
            let runtime = leaf.runtime();
            if !ctx.udp_enabled_for_outbound(runtime.udp_policy_tag.as_deref()) {
                return Err(FlowFailure {
                    stage: "udp_policy",
                    error: EngineError::Io(std::io::Error::other(
                        "udp disabled for relay chain outbound",
                    )),
                    upstream: runtime.endpoint.map(|endpoint| endpoint.upstream()),
                });
            }
        }
        Ok(())
    }

    fn prepare_udp_relay_two_stream_operation<'a>(
        &self,
        claimed: ClaimedOutboundLeaf<'a>,
        ctx: UdpAdapterContext<'a>,
        post_carrier: RelayCarrier,
        get_carrier: RelayCarrier,
    ) -> Result<Box<dyn PreparedUdpFlowOperation + 'a>, FlowFailure> {
        claimed.prepare_owned_udp_relay_two_stream(post_carrier, get_carrier, ctx.source_dir())
    }

    fn prepare_udp_relay_final_hop_operation<'a>(
        &self,
        claimed: ClaimedOutboundLeaf<'a>,
        ctx: UdpAdapterContext<'a>,
        carrier: RelayCarrier,
    ) -> Result<Box<dyn PreparedUdpFlowOperation + 'a>, FlowFailure> {
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
