use std::time::Duration;

use tokio::time::Instant as TokioInstant;

use zero_engine::EngineError;

use crate::runtime::udp_dispatch::FlowFailure;
use crate::runtime::udp_flow::managed::{
    ManagedStreamFlowSender, ManagedUdpFlowKind, ManagedUdpFlowRequest, ManagedUdpFlowResume,
    ManagedUdpHandlers, ManagedUdpState,
};
use crate::runtime::udp_flow::outbound::ManagedUdpFlowRef;
use crate::runtime::udp_flow::response::UpstreamUdpResponse;

use upstream::UpstreamAssociationState;
pub(crate) use upstream::{UpstreamAssociationHandler, UpstreamUdpHandlers};

mod forward;
mod upstream;

pub(crate) struct RegisteredUpstreamAssociationView<'a> {
    pub(crate) outbound_tag: &'a str,
}

pub(crate) struct ClosedRegisteredUpstreamAssociation {
    pub(crate) outbound_tag: String,
    pub(crate) server: String,
    pub(crate) port: u16,
}

pub(crate) struct RegisteredUdpState {
    pub(super) managed: ManagedUdpState,
    upstream: UpstreamAssociationState,
}

pub(crate) struct RegisteredUdpHandlers {
    pub(crate) managed: ManagedUdpHandlers,
    pub(crate) upstream: UpstreamUdpHandlers,
}

impl RegisteredUdpState {
    pub(crate) fn new(handlers: RegisteredUdpHandlers) -> Self {
        Self {
            managed: ManagedUdpState::new(handlers.managed),
            upstream: UpstreamAssociationState::new(handlers.upstream),
        }
    }

    pub(crate) fn register_managed_flow(
        &mut self,
        resume: ManagedUdpFlowResume,
    ) -> ManagedUdpFlowRef {
        self.managed.register_flow(resume)
    }

    pub(crate) fn register_managed_stream_flow_sender(
        &mut self,
        sender: Box<dyn ManagedStreamFlowSender>,
    ) -> ManagedUdpFlowRef {
        self.managed.register_stream_sender(sender)
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

fn closed_registered_upstream_association(
    (outbound_tag, server, port): (String, String, u16),
) -> ClosedRegisteredUpstreamAssociation {
    ClosedRegisteredUpstreamAssociation {
        outbound_tag,
        server,
        port,
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
