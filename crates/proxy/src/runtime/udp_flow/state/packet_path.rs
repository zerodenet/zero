use crate::protocol_registry::UdpAdapterContext;
use crate::runtime::udp_flow::packet_path::{UdpFlowContext, UdpPacketRef};
use crate::runtime::udp_flow::packet_path_chain::{
    PacketPathStartRequest, SendWithSnapshotRequest,
};
use crate::runtime::udp_flow::result::FlowFailure;
use crate::runtime::udp_flow::snapshot::UdpFlowSnapshot;

use super::UdpFlowState;

impl UdpFlowState {
    pub(crate) async fn send_packet_path_chain(
        &mut self,
        ctx: UdpAdapterContext<'_>,
        request: PacketPathStartRequest<'_>,
    ) -> Result<usize, FlowFailure> {
        self.packet_path
            .send(
                UdpFlowContext {
                    chain_tasks: &mut self.chain_tasks,
                    session_id: request.session_id,
                },
                ctx,
                request,
            )
            .await
    }

    pub(crate) async fn forward_existing_packet_path_flow(
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
