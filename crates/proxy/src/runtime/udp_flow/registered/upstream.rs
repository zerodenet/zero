use std::time::Duration;

use async_trait::async_trait;
use tokio::time::Instant as TokioInstant;
use zero_engine::EngineError;

use crate::runtime::udp_dispatch::FlowFailure;
use crate::runtime::udp_flow::managed::{ManagedUdpFlowRequest, ManagedUdpFlowResume};
use crate::runtime::udp_flow::response::UpstreamUdpResponse;

#[async_trait]
pub(crate) trait UpstreamAssociationHandler: Send + Sync {
    fn supports_upstream_resume(&self, resume: &ManagedUdpFlowResume) -> bool;

    async fn send_upstream(
        &mut self,
        inbound_tag: &str,
        request: ManagedUdpFlowRequest<'_>,
    ) -> Result<usize, FlowFailure>;

    async fn recv_upstream_response(
        &self,
        buf: &mut [u8],
    ) -> Result<UpstreamUdpResponse, EngineError>;

    async fn recv_raw_upstream_packet(&self, buf: &mut [u8]) -> Result<usize, EngineError>;

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

pub(super) struct UpstreamAssociationState {
    handlers: UpstreamUdpHandlers,
}

impl UpstreamAssociationState {
    pub(super) fn new(handlers: UpstreamUdpHandlers) -> Self {
        Self { handlers }
    }

    pub(super) fn handles_resume(&self, resume: &ManagedUdpFlowResume) -> bool {
        self.handlers
            .upstream
            .iter()
            .any(|handler| handler.supports_upstream_resume(resume))
    }

    pub(super) async fn start_upstream_flow(
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

    pub(super) async fn recv_upstream_response(
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

    pub(super) async fn recv_raw_upstream_packet(
        &self,
        buf: &mut [u8],
    ) -> Result<usize, EngineError> {
        for handler in &self.handlers.upstream {
            if handler.upstream_outbound_tag().is_some() {
                return handler.recv_raw_upstream_packet(buf).await;
            }
        }
        std::future::pending::<Result<usize, EngineError>>().await
    }

    pub(super) fn upstream_outbound_tag(&self) -> Option<&str> {
        self.handlers
            .upstream
            .iter()
            .find_map(|handler| handler.upstream_outbound_tag())
    }

    pub(super) fn upstream_idle_deadline(&self) -> Option<TokioInstant> {
        self.handlers
            .upstream
            .iter()
            .filter_map(|handler| handler.upstream_idle_deadline())
            .min()
    }

    pub(super) fn touch_upstream_idle(&mut self, timeout: Duration) {
        for handler in &mut self.handlers.upstream {
            if handler.upstream_outbound_tag().is_some() {
                handler.touch_upstream_idle(timeout);
            }
        }
    }

    pub(super) fn drop_upstream_association(&mut self) -> Option<(String, String, u16)> {
        self.handlers
            .upstream
            .iter_mut()
            .find_map(|handler| handler.drop_upstream_association())
    }

    pub(super) fn close_idle_upstream(&mut self) -> Option<(String, String, u16)> {
        self.handlers
            .upstream
            .iter_mut()
            .find_map(|handler| handler.close_idle_upstream())
    }

    pub(super) fn close_all_upstreams(&mut self) {
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
