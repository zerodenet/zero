use zero_core::Session;
use zero_engine::ResolvedLeafOutbound;

use crate::protocol_runtime::udp::packet_path_traits::{UdpFlowContext, UdpPacketRef};
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::runtime::Proxy;

impl UdpDispatch {
    pub(super) async fn start_relay_flow(
        &mut self,
        proxy: &Proxy,
        chain: Vec<ResolvedLeafOutbound<'_>>,
        session: &Session,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        // Datagram-over-packet-path: carrier provides a raw send/recv channel,
        // datagram encodes through it. Resolve both positions via the adapter
        // registry; no match on the protocol enum.
        if chain.len() == 2 {
            let carrier_leaf = &chain[0];
            let datagram_leaf = &chain[1];
            if let Some(flow_binding) = proxy
                .protocols
                .udp_packet_path_pair(carrier_leaf, datagram_leaf)
            {
                let sent = self
                    .protocol_state
                    .send_packet_path_chain(
                        UdpFlowContext {
                            chain_tasks: &mut self.chain_tasks,
                            session_id: session.id,
                        },
                        proxy,
                        carrier_leaf,
                        datagram_leaf,
                        UdpPacketRef {
                            target: &session.target,
                            port: session.port,
                            payload,
                        },
                    )
                    .await?;

                return Ok(FlowStartResult::Flow {
                    outbound: Box::new(
                        self.protocol_state
                            .datagram_chain_flow_outbound(flow_binding),
                    ),
                    tx_bytes: sent as u64,
                });
            }
        }

        let final_hop = chain.last().expect("relay chain has at least 2 hops");

        // Two-stream XHTTP path (VLESS legacy split_http packet-up/stream-up):
        // ProtocolInventory resolves the final hop adapter. stream-one / auto
        // fall through to the generic single-stream path below.
        if proxy
            .protocols
            .udp_relay_needs_two_streams(final_hop)
            .map_err(|error| FlowFailure {
                stage: "find_outbound_leaf",
                error,
                upstream: None,
            })?
        {
            return proxy
                .protocols
                .start_udp_relay_two_stream(self, proxy, session, chain, payload)
                .await;
        }

        // Generic single-stream path: run the relay prefix once, then apply the
        // final hop protocol over the carrier stream.
        let (carrier, final_hop) =
            proxy
                .dispatch_tcp_relay_prefix(chain)
                .await
                .map_err(|failure| FlowFailure {
                    stage: failure.stage,
                    error: failure.error,
                    upstream: failure.upstream_endpoint,
                })?;

        proxy
            .protocols
            .start_udp_relay_final_hop(self, proxy, session, carrier, &final_hop, payload)
            .await
    }
}
