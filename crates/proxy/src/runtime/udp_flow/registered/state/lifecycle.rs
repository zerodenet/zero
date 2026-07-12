use std::time::Duration;

use tokio::time::Instant as TokioInstant;
use zero_engine::EngineError;

use super::model::{
    ClosedRegisteredUpstreamAssociation, RegisteredUdpHandlers, RegisteredUdpState,
    RegisteredUpstreamAssociationView,
};
use crate::runtime::udp_flow::managed::ManagedUdpFlowResume;
use crate::runtime::udp_flow::outbound::ManagedUdpFlowRef;
use crate::runtime::udp_flow::response::UpstreamUdpResponse;

impl RegisteredUdpState {
    pub(crate) fn new(handlers: RegisteredUdpHandlers) -> Self {
        Self {
            managed: super::super::super::managed::ManagedUdpState::new(handlers.managed),
            upstream: super::super::upstream::UpstreamAssociationState::new(handlers.upstream),
        }
    }

    pub(crate) fn register_managed_flow(
        &mut self,
        resume: ManagedUdpFlowResume,
    ) -> ManagedUdpFlowRef {
        self.managed.register_flow(resume)
    }

    pub(crate) fn managed_flow_resume(
        &self,
        flow_ref: ManagedUdpFlowRef,
    ) -> Option<ManagedUdpFlowResume> {
        self.managed.flow_resume(flow_ref)
    }

    pub(crate) async fn recv_upstream_response(
        &self,
        buf: &mut [u8],
    ) -> Result<UpstreamUdpResponse, EngineError> {
        self.upstream.recv_upstream_response(buf).await
    }

    pub(crate) fn upstream_association_view(
        &self,
    ) -> Option<RegisteredUpstreamAssociationView<'_>> {
        self.upstream
            .upstream_outbound_tag()
            .map(|outbound_tag| RegisteredUpstreamAssociationView { outbound_tag })
    }

    pub(crate) fn upstream_idle_deadline(&self) -> Option<TokioInstant> {
        self.upstream.upstream_idle_deadline()
    }

    pub(crate) fn touch_upstream_idle(&mut self, timeout: Duration) {
        self.upstream.touch_upstream_idle(timeout);
    }

    pub(crate) fn drop_upstream_association(
        &mut self,
    ) -> Option<ClosedRegisteredUpstreamAssociation> {
        self.upstream
            .drop_upstream_association()
            .map(closed_registered_upstream_association)
    }

    pub(crate) fn close_idle_upstream(&mut self) -> Option<ClosedRegisteredUpstreamAssociation> {
        self.upstream
            .close_idle_upstream()
            .map(closed_registered_upstream_association)
    }

    pub(crate) fn close_all_upstreams(mut self) {
        self.upstream.close_all_upstreams();
    }
}

fn closed_registered_upstream_association(
    (outbound_tag, server, port): (String, String, u16),
) -> ClosedRegisteredUpstreamAssociation {
    ClosedRegisteredUpstreamAssociation {
        outbound_tag,
        server,
        port,
    }
}
