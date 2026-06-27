use std::time::Duration;

use async_trait::async_trait;
use tokio::time::Instant as TokioInstant;
use zero_engine::EngineError;

use crate::protocol_runtime::udp::FlowFailure;
use crate::runtime::udp_flow::managed::{ManagedUdpFlowRequest, ManagedUdpFlowResume};

#[async_trait]
pub(crate) trait UpstreamAssociationHandler: Send + Sync {
    fn supports_upstream_resume(&self, resume: &ManagedUdpFlowResume) -> bool;

    async fn send_upstream(
        &mut self,
        inbound_tag: &str,
        request: ManagedUdpFlowRequest<'_>,
    ) -> Result<usize, FlowFailure>;

    async fn recv_upstream_packet(&self, buf: &mut [u8]) -> Result<usize, EngineError>;

    fn upstream_outbound_tag(&self) -> Option<&str>;

    fn upstream_idle_deadline(&self) -> Option<TokioInstant>;

    fn touch_upstream_idle(&mut self, timeout: Duration);

    fn drop_upstream_association(&mut self) -> Option<(String, String, u16)>;

    fn close_idle_upstream(&mut self) -> Option<(String, String, u16)>;

    fn close_all_upstreams(&mut self);
}

pub(crate) struct UpstreamUdpHandlers {
    pub(crate) upstream: Vec<Box<dyn UpstreamAssociationHandler>>,
}

pub(in crate::protocol_runtime::udp::state) struct UpstreamAssociationState {
    handlers: UpstreamUdpHandlers,
}

impl UpstreamAssociationState {
    pub(in crate::protocol_runtime::udp::state) fn new(handlers: UpstreamUdpHandlers) -> Self {
        Self { handlers }
    }

    pub(in crate::protocol_runtime::udp::state) fn handles_resume(
        &self,
        resume: &ManagedUdpFlowResume,
    ) -> bool {
        self.handlers
            .upstream
            .iter()
            .any(|handler| handler.supports_upstream_resume(resume))
    }

    pub(in crate::protocol_runtime::udp::state) async fn start_upstream_flow(
        &mut self,
        inbound_tag: &str,
        request: ManagedUdpFlowRequest<'_>,
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

    pub(in crate::protocol_runtime::udp::state) async fn recv_upstream_packet(
        &self,
        buf: &mut [u8],
    ) -> Result<usize, EngineError> {
        for handler in &self.handlers.upstream {
            if handler.upstream_outbound_tag().is_some() {
                return handler.recv_upstream_packet(buf).await;
            }
        }
        std::future::pending::<Result<usize, EngineError>>().await
    }

    pub(in crate::protocol_runtime::udp::state) fn upstream_outbound_tag(&self) -> Option<&str> {
        self.handlers
            .upstream
            .iter()
            .find_map(|handler| handler.upstream_outbound_tag())
    }

    pub(in crate::protocol_runtime::udp::state) fn upstream_idle_deadline(
        &self,
    ) -> Option<TokioInstant> {
        self.handlers
            .upstream
            .iter()
            .filter_map(|handler| handler.upstream_idle_deadline())
            .min()
    }

    pub(in crate::protocol_runtime::udp::state) fn touch_upstream_idle(
        &mut self,
        timeout: Duration,
    ) {
        for handler in &mut self.handlers.upstream {
            if handler.upstream_outbound_tag().is_some() {
                handler.touch_upstream_idle(timeout);
            }
        }
    }

    pub(in crate::protocol_runtime::udp::state) fn drop_upstream_association(
        &mut self,
    ) -> Option<(String, String, u16)> {
        self.handlers
            .upstream
            .iter_mut()
            .find_map(|handler| handler.drop_upstream_association())
    }

    pub(in crate::protocol_runtime::udp::state) fn close_idle_upstream(
        &mut self,
    ) -> Option<(String, String, u16)> {
        self.handlers
            .upstream
            .iter_mut()
            .find_map(|handler| handler.close_idle_upstream())
    }

    pub(in crate::protocol_runtime::udp::state) fn close_all_upstreams(&mut self) {
        for handler in &mut self.handlers.upstream {
            handler.close_all_upstreams();
        }
    }
}

fn upstream_flow_mismatch(
    stage: &'static str,
    server: &str,
    port: u16,
    message: &'static str,
) -> FlowFailure {
    FlowFailure {
        stage,
        error: EngineError::Io(std::io::Error::other(message)),
        upstream: Some((server.to_string(), port)),
    }
}
