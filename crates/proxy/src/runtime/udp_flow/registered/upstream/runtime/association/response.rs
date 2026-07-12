use zero_engine::EngineError;

use super::super::super::contract::{UpstreamAssociationTarget, UpstreamAssociationTransport};
use super::model::UpstreamAssociationRuntime;
use crate::runtime::udp_flow::response::UpstreamUdpResponse;

impl<T, A> UpstreamAssociationRuntime<T, A>
where
    T: UpstreamAssociationTarget,
    A: UpstreamAssociationTransport<T>,
{
    pub(crate) async fn recv_upstream_response(
        &self,
        buf: &mut [u8],
    ) -> Result<UpstreamUdpResponse, EngineError> {
        if let Some(association) = self.upstream.association() {
            let (target, port, payload) = association.recv_response_parts(buf).await?;
            return Ok(UpstreamUdpResponse::new(target, port, payload));
        }
        std::future::pending::<Result<UpstreamUdpResponse, EngineError>>().await
    }
}
