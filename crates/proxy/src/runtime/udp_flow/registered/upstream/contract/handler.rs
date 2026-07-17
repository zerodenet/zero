use std::time::Duration;

use async_trait::async_trait;
use tokio::time::Instant as TokioInstant;
use zero_engine::EngineError;

use super::model::UpstreamAssociationSend;
use crate::runtime::udp_flow::managed::ManagedUdpFlowResume;
use crate::runtime::udp_flow::response::UpstreamUdpResponse;
use crate::runtime::udp_flow::result::FlowFailure;

#[async_trait]
pub(crate) trait UpstreamAssociationHandler: Send + Sync {
    fn supports_upstream_resume(&self, resume: &ManagedUdpFlowResume) -> bool;

    async fn send_upstream(
        &mut self,
        inbound_tag: &str,
        request: UpstreamAssociationSend<'_>,
    ) -> Result<usize, FlowFailure>;

    async fn recv_upstream_response(
        &self,
        buf: &mut [u8],
    ) -> Result<UpstreamUdpResponse, EngineError>;

    fn upstream_outbound_tag(&self) -> Option<&str>;

    fn upstream_idle_deadline(&self) -> Option<TokioInstant>;

    fn touch_upstream_idle(&mut self, timeout: Duration);

    #[cfg(feature = "upstream-association-runtime")]
    fn drop_upstream_association(&mut self) -> Option<(String, String, u16)>;

    #[cfg(feature = "upstream-association-runtime")]
    fn close_idle_upstream(&mut self) -> Option<(String, String, u16)>;

    fn close_all_upstreams(&mut self);
}
