use super::super::super::super::model::ManagedStreamExistingSend;
use super::super::super::connector::ManagedStreamFlowConnector;
use super::super::mismatch::managed_mismatch;
use super::super::model::ManagedStreamFlowManager;
use crate::runtime::path::OutboundEndpoint;
use crate::runtime::udp_flow::packet_path::{UdpFlowContext, UdpPacketRef};
use crate::runtime::udp_flow::result::FlowFailure;

impl<T> ManagedStreamFlowManager<T>
where
    T: ManagedStreamFlowConnector,
{
    pub(in super::super) async fn send_managed_existing(
        &mut self,
        request: ManagedStreamExistingSend<'_>,
    ) -> Result<usize, FlowFailure> {
        let Some(resume) = request.resume.cloned::<T>() else {
            return Err(managed_mismatch(
                self.mismatch_stage,
                request.server,
                request.port,
                self.mismatch_message,
            ));
        };
        self.send(
            UdpFlowContext {
                chain_tasks: request.chain_tasks,
                session_id: request.session_id,
            },
            request.services,
            request.session,
            OutboundEndpoint {
                server: request.server.to_owned(),
                port: request.port,
            },
            resume,
            UdpPacketRef {
                target: request.target,
                port: request.target_port,
                payload: request.payload,
            },
        )
        .await
    }
}
