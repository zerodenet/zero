use crate::protocol_runtime::udp::ProtocolUdpState;
use crate::protocol_runtime::udp::{FlowFailure, ManagedUdpFlowRequest, ProtocolUdpFlowResume};
use crate::runtime::udp_flow::outbound::ManagedUdpFlowRef;
use crate::runtime::udp_flow::packet_path::ChainTask;
use tokio::task::JoinSet;

use super::UdpDispatch;

impl UdpDispatch {
    pub(crate) fn protocol_udp_state_and_chain_tasks(
        &mut self,
    ) -> (&mut ProtocolUdpState, &mut JoinSet<ChainTask>) {
        (&mut self.protocol_state, &mut self.chain_tasks)
    }

    pub(crate) async fn start_managed_protocol_flow(
        &mut self,
        mut request: ManagedUdpFlowRequest<'_>,
    ) -> Result<usize, FlowFailure> {
        request.chain_tasks = Some(&mut self.chain_tasks);
        self.protocol_state
            .start_managed_udp_flow(&self.inbound_tag, request)
            .await
    }

    pub(crate) fn register_managed_protocol_flow(
        &mut self,
        resume: ProtocolUdpFlowResume,
    ) -> ManagedUdpFlowRef {
        self.protocol_state.register_managed_flow(resume)
    }

    pub(crate) fn managed_protocol_flow_resume(
        &self,
        flow_ref: ManagedUdpFlowRef,
    ) -> Option<ProtocolUdpFlowResume> {
        self.protocol_state.managed_flow_resume(flow_ref)
    }
}
