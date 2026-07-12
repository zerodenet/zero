use std::time::Duration;

use async_trait::async_trait;
use tokio::time::Instant as TokioInstant;

use super::super::contract::{
    handles_registered_resume, UpstreamAssociationHandler, UpstreamAssociationStages,
    UpstreamAssociationTarget, UpstreamAssociationTransport,
};
use super::association::UpstreamAssociationRuntime;
use super::control::{
    close_registered_dropped_upstream, close_registered_idle_upstream,
    start_registered_upstream_flow,
};
use crate::runtime::udp_flow::managed::{ManagedUdpFlowRequest, ManagedUdpFlowResume};
use crate::runtime::udp_flow::response::UpstreamUdpResponse;
use zero_engine::EngineError;

pub(crate) struct RegisteredUpstreamAssociationHandler<T, A> {
    runtime: UpstreamAssociationRuntime<T, A>,
    stages: UpstreamAssociationStages,
}

impl<T, A> RegisteredUpstreamAssociationHandler<T, A> {
    pub(crate) fn new(stages: UpstreamAssociationStages) -> Self {
        Self {
            runtime: UpstreamAssociationRuntime::<T, A>::default(),
            stages,
        }
    }
}

#[async_trait]
impl<T, A> UpstreamAssociationHandler for RegisteredUpstreamAssociationHandler<T, A>
where
    T: UpstreamAssociationTarget + Send + Sync + 'static,
    A: UpstreamAssociationTransport<T> + 'static,
{
    fn supports_upstream_resume(&self, resume: &ManagedUdpFlowResume) -> bool {
        handles_registered_resume::<T>(resume)
    }

    async fn send_upstream(
        &mut self,
        inbound_tag: &str,
        request: ManagedUdpFlowRequest<'_>,
    ) -> Result<usize, crate::runtime::udp_dispatch::FlowFailure> {
        start_registered_upstream_flow(
            &mut self.runtime,
            inbound_tag,
            request,
            self.stages.proxy_stage,
            self.stages.resume_stage,
            self.stages.resume_message,
        )
        .await
    }

    async fn recv_upstream_response(
        &self,
        buf: &mut [u8],
    ) -> Result<UpstreamUdpResponse, EngineError> {
        self.runtime.recv_upstream_response(buf).await
    }

    fn upstream_outbound_tag(&self) -> Option<&str> {
        self.runtime.upstream_outbound_tag()
    }

    fn upstream_idle_deadline(&self) -> Option<TokioInstant> {
        self.runtime.idle_deadline()
    }

    fn touch_upstream_idle(&mut self, timeout: Duration) {
        self.runtime.touch_idle(timeout);
    }

    fn drop_upstream_association(&mut self) -> Option<(String, String, u16)> {
        close_registered_dropped_upstream(&mut self.runtime)
    }

    fn close_idle_upstream(&mut self) -> Option<(String, String, u16)> {
        close_registered_idle_upstream(&mut self.runtime)
    }

    fn close_all_upstreams(&mut self) {
        self.runtime.close_all_upstreams();
    }
}

pub(crate) fn boxed_registered_upstream_handler<T, A>(
    stages: UpstreamAssociationStages,
) -> Box<dyn UpstreamAssociationHandler>
where
    T: UpstreamAssociationTarget + Send + Sync + 'static,
    A: UpstreamAssociationTransport<T> + 'static,
{
    Box::new(RegisteredUpstreamAssociationHandler::<T, A>::new(stages))
}
