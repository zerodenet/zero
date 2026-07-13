use zero_engine::EngineError;

use super::super::super::runtime::upstream_flow_mismatch;
use super::model::UpstreamAssociationState;
use crate::runtime::udp_flow::managed::ManagedUdpFlowResume;
use crate::runtime::udp_flow::registered::upstream::UpstreamAssociationSend;
use crate::runtime::udp_flow::response::UpstreamUdpResponse;
use crate::runtime::udp_flow::result::FlowFailure;

impl UpstreamAssociationState {
    pub(in crate::runtime::udp_flow::registered) fn handles_resume(
        &self,
        resume: &ManagedUdpFlowResume,
    ) -> bool {
        self.handlers
            .upstream
            .iter()
            .any(|handler| handler.supports_upstream_resume(resume))
    }

    pub(in crate::runtime::udp_flow::registered) async fn start_upstream_flow(
        &mut self,
        inbound_tag: &str,
        request: UpstreamAssociationSend<'_>,
    ) -> Result<usize, FlowFailure> {
        for handler in &mut self.handlers.upstream {
            if !handler.supports_upstream_resume(&request.resume) {
                continue;
            }
            return handler.send_upstream(inbound_tag, request).await;
        }
        Err(upstream_flow_mismatch(
            "udp_upstream_resume",
            request.server,
            request.port,
            "expected registered upstream UDP association handler",
        ))
    }

    pub(in crate::runtime::udp_flow::registered) async fn recv_upstream_response(
        &self,
        buf: &mut [u8],
    ) -> Result<UpstreamUdpResponse, EngineError> {
        for handler in &self.handlers.upstream {
            if handler.upstream_outbound_tag().is_some() {
                return handler.recv_upstream_response(buf).await;
            }
        }
        std::future::pending::<Result<UpstreamUdpResponse, EngineError>>().await
    }
}
