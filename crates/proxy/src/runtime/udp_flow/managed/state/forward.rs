use super::model::ManagedUdpState;
use crate::protocol_registry::UdpRuntimeServices;
use crate::runtime::udp_flow::managed::flow::{ManagedExistingFlowForward, ManagedUdpFlowResume};
use crate::runtime::udp_flow::packet_path::ChainTask;
use crate::runtime::udp_flow::result::FlowFailure;
use tokio::task::JoinSet;

impl ManagedUdpState {
    pub(crate) async fn forward_existing_flow(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        services: UdpRuntimeServices,
        request: ManagedExistingFlowForward<'_>,
        resume: ManagedUdpFlowResume,
    ) -> Result<Option<usize>, FlowFailure> {
        let (flow, payload) = request;
        #[cfg(feature = "managed-datagram-runtime")]
        if let Some(result) = self
            .datagram
            .forward_existing_flow(chain_tasks, services.clone(), flow, &resume, payload)
            .await
        {
            return result.map(Some);
        }
        #[cfg(feature = "managed-stream-runtime")]
        if let Some(result) = self
            .stream
            .forward_existing_flow(chain_tasks, services, flow, &resume, payload)
            .await
        {
            return result.map(Some);
        }

        Ok(None)
    }
}
