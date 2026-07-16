use super::super::super::error::flow_mismatch;
use super::super::super::model::ManagedUdpState;
use crate::runtime::udp_flow::managed::flow::{ManagedRelayStreamFlow, ManagedUdpFlowRequest};
use crate::runtime::udp_flow::result::FlowFailure;

impl ManagedUdpState {
    pub(crate) async fn start_relay_stream_request(
        &mut self,
        request: ManagedUdpFlowRequest<'_>,
    ) -> Result<Option<usize>, FlowFailure> {
        let Some(carrier) = request.carrier else {
            return Ok(None);
        };
        let Some(chain_tasks) = request.chain_tasks else {
            return Err(flow_mismatch(
                "udp_managed_flow_chain_tasks",
                request.server,
                request.port,
                "expected chain task context for managed UDP flow",
            ));
        };
        self.start_relay_stream_flow(ManagedRelayStreamFlow {
            chain_tasks,
            services: request.services,
            session: request.session,
            carrier,
            tls_server_name: request.tls_server_name,
            server: request.server,
            port: request.port,
            resume: request.resume,
            payload: request.payload,
        })
        .await
        .map(Some)
    }
}
