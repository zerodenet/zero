use zero_engine::EngineError;

use super::model::RegisteredUdpState;
use crate::runtime::udp_dispatch::FlowFailure;
use crate::runtime::udp_flow::managed::{ManagedUdpFlowKind, ManagedUdpFlowRequest};

impl RegisteredUdpState {
    pub(crate) async fn start_managed_udp_flow(
        &mut self,
        inbound_tag: &str,
        request: ManagedUdpFlowRequest<'_>,
    ) -> Result<usize, FlowFailure> {
        if matches!(request.kind, ManagedUdpFlowKind::RelayStream) && request.carrier.is_none() {
            return self
                .upstream
                .start_upstream_flow(inbound_tag, request)
                .await;
        }
        let result = self.managed.start_flow(request).await?;
        if let Some(sent) = result {
            return Ok(sent);
        }
        Err(unhandled_managed_flow())
    }
}

fn unhandled_managed_flow() -> FlowFailure {
    FlowFailure {
        stage: "udp_managed_flow_start",
        error: EngineError::Io(std::io::Error::other(
            "managed UDP flow request had no compiled start handler",
        )),
        upstream: None,
    }
}
