#[cfg(any(
    feature = "managed-stream-runtime",
    feature = "managed-datagram-runtime"
))]
use crate::protocol_registry::UdpRuntimeServices;
#[cfg(any(
    feature = "managed-stream-runtime",
    feature = "managed-datagram-runtime"
))]
use crate::runtime::udp_flow::managed::ManagedExistingFlowForward;
#[cfg(any(
    feature = "managed-stream-runtime",
    feature = "managed-datagram-runtime"
))]
use crate::runtime::udp_flow::managed::ManagedUdpFlowRequest;
use crate::runtime::udp_flow::managed::ManagedUdpFlowResume;
use crate::runtime::udp_flow::outbound::ManagedUdpFlowRef;
#[cfg(feature = "upstream-association-runtime")]
use crate::runtime::udp_flow::registered::UpstreamAssociationSend;
use crate::runtime::udp_flow::result::FlowFailure;

use super::UdpFlowState;

impl UdpFlowState {
    #[cfg(any(
        feature = "managed-stream-runtime",
        feature = "managed-datagram-runtime"
    ))]
    pub(crate) async fn start_managed_flow(
        &mut self,
        inbound_tag: &str,
        mut request: ManagedUdpFlowRequest<'_>,
    ) -> Result<usize, FlowFailure> {
        request.chain_tasks = Some(&mut self.chain_tasks);
        self.registered
            .start_managed_udp_flow(inbound_tag, request)
            .await
    }

    #[cfg(feature = "upstream-association-runtime")]
    pub(crate) async fn start_upstream_flow(
        &mut self,
        inbound_tag: &str,
        request: UpstreamAssociationSend<'_>,
    ) -> Result<usize, FlowFailure> {
        self.registered
            .start_upstream_udp_flow(inbound_tag, request)
            .await
    }

    #[cfg(feature = "upstream-association-runtime")]
    pub(crate) fn handles_upstream_resume(&self, resume: &ManagedUdpFlowResume) -> bool {
        self.registered.handles_upstream_resume(resume)
    }

    pub(crate) fn register_managed_flow(
        &mut self,
        resume: ManagedUdpFlowResume,
    ) -> ManagedUdpFlowRef {
        self.registered.register_managed_flow(resume)
    }

    #[cfg(any(
        feature = "upstream-association-runtime",
        feature = "managed-stream-runtime"
    ))]
    pub(crate) fn managed_flow_resume(
        &self,
        flow_ref: ManagedUdpFlowRef,
    ) -> Option<ManagedUdpFlowResume> {
        self.registered.managed_flow_resume(flow_ref)
    }

    #[cfg(any(
        feature = "managed-stream-runtime",
        feature = "managed-datagram-runtime"
    ))]
    pub(crate) async fn forward_existing_managed_flow(
        &mut self,
        services: UdpRuntimeServices,
        request: ManagedExistingFlowForward<'_>,
    ) -> Result<usize, FlowFailure> {
        self.registered
            .forward_existing_managed_flow(&mut self.chain_tasks, services, request)
            .await
    }
}
