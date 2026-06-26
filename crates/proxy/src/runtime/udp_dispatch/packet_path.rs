use zero_engine::ResolvedLeafOutbound;

use super::{FlowFailure, UdpDispatch};
use crate::runtime::udp_flow::outbound::UdpFlowOutbound;
use crate::runtime::udp_flow::packet_path::{PacketPathFlowBinding, UdpFlowContext, UdpPacketRef};
use crate::runtime::udp_flow::packet_path_chain::SendWithSnapshotRequest;
use crate::runtime::udp_flow::sessions::UdpFlowSnapshot;
use crate::runtime::Proxy;

impl UdpDispatch {
    pub(super) fn datagram_chain_flow_outbound(
        flow_binding: PacketPathFlowBinding<'_>,
    ) -> UdpFlowOutbound {
        let (datagram, flow_snapshot) = flow_binding.into_parts();
        let descriptor = datagram.descriptor();
        let tag = descriptor.tag.to_owned();
        let server = descriptor.server.to_owned();
        let port = descriptor.port;

        UdpFlowOutbound::PacketPathDatagram {
            tag,
            server,
            port,
            snapshot: flow_snapshot,
        }
    }

    pub(super) async fn send_packet_path_chain(
        &mut self,
        session_id: u64,
        proxy: &Proxy,
        carrier_leaf: &ResolvedLeafOutbound<'_>,
        datagram_leaf: &ResolvedLeafOutbound<'_>,
        packet: UdpPacketRef<'_>,
    ) -> Result<usize, FlowFailure> {
        self.packet_path
            .send(
                UdpFlowContext {
                    chain_tasks: &mut self.chain_tasks,
                    session_id,
                },
                proxy,
                carrier_leaf,
                datagram_leaf,
                packet,
            )
            .await
    }

    pub(super) async fn forward_existing_packet_path_flow(
        &mut self,
        flow: &UdpFlowSnapshot,
        payload: &[u8],
    ) -> Result<usize, FlowFailure> {
        let snapshot = flow
            .outbound
            .packet_path_snapshot()
            .expect("packet-path flow should expose packet-path snapshot");
        self.packet_path
            .send_with_snapshot(SendWithSnapshotRequest {
                ctx: UdpFlowContext {
                    chain_tasks: &mut self.chain_tasks,
                    session_id: flow.session.id,
                },
                lookup_key: snapshot.lookup_key(),
                packet_ref: UdpPacketRef {
                    target: &flow.session.target,
                    port: flow.session.port,
                    payload,
                },
            })
            .await
    }
}
