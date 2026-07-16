use super::super::super::error::flow_mismatch;
use super::super::super::model::ManagedUdpState;
use crate::runtime::udp_flow::managed::flow::{ManagedStreamPacketFlow, ManagedUdpFlowRequest};
use crate::runtime::udp_flow::result::FlowFailure;

impl ManagedUdpState {
    pub(crate) async fn start_stream_packet_request(
        &mut self,
        request: ManagedUdpFlowRequest<'_>,
    ) -> Result<Option<usize>, FlowFailure> {
        let Some(chain_tasks) = request.chain_tasks else {
            return Err(flow_mismatch(
                "udp_managed_flow_chain_tasks",
                request.server,
                request.port,
                "expected chain task context for managed UDP flow",
            ));
        };
        let Some(services) = request.services else {
            return Err(flow_mismatch(
                "udp_stream_packet_proxy",
                request.server,
                request.port,
                "expected proxy context for stream-packet UDP flow",
            ));
        };
        self.start_stream_packet_flow(ManagedStreamPacketFlow {
            chain_tasks,
            services,
            session: request.session,
            server: request.server,
            port: request.port,
            resume: request.resume,
            payload: request.payload,
        })
        .await
        .map(Some)
    }
}
