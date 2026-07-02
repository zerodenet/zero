use std::time::Duration;

use async_trait::async_trait;
use tokio::time::Instant as TokioInstant;
use zero_core::{Address, Session};
use zero_engine::EngineError;

use crate::runtime::udp_dispatch::FlowFailure;
use crate::runtime::udp_flow::managed::{ManagedUdpFlowRequest, ManagedUdpFlowResume};
use crate::runtime::udp_flow::response::UpstreamUdpResponse;
use crate::runtime::Proxy;

pub(crate) struct TrackedUpstreamAssociation<T, A> {
    target: T,
    association: A,
}

pub(crate) struct TrackedUpstreamAssociationState<T, A> {
    upstream: Option<TrackedUpstreamAssociation<T, A>>,
}

pub(crate) trait UpstreamAssociationTarget: Clone + PartialEq {
    fn outbound_tag(&self) -> &str;

    fn log_parts(&self) -> (&str, &str, u16);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UpstreamAssociationCloseReason {
    Closed,
    IdleTimeout,
    Dropped,
}

#[async_trait]
pub(crate) trait UpstreamAssociationTransport<T>: Send + Sync + Sized
where
    T: UpstreamAssociationTarget,
{
    async fn establish(proxy: &Proxy, target: T, session_id: u64) -> Result<Self, EngineError>;

    async fn send_packet(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, EngineError>;

    async fn recv_response_parts(
        &self,
        buf: &mut [u8],
    ) -> Result<(Address, u16, Vec<u8>), EngineError>;

    fn close(self, reason: UpstreamAssociationCloseReason);
}

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

pub(crate) struct UpstreamAssociationRuntime<T, A> {
    upstream: TrackedUpstreamAssociationState<T, A>,
    idle_deadline: Option<TokioInstant>,
}

impl<T, A> TrackedUpstreamAssociation<T, A> {
    pub(crate) fn new(target: T, association: A) -> Self {
        Self {
            target,
            association,
        }
    }

    pub(crate) fn target(&self) -> &T {
        &self.target
    }

    pub(crate) fn association(&self) -> &A {
        &self.association
    }

    pub(crate) fn into_parts(self) -> (T, A) {
        (self.target, self.association)
    }
}

impl<T, A> TrackedUpstreamAssociationState<T, A> {
    pub(crate) fn new() -> Self {
        Self { upstream: None }
    }

    pub(crate) fn target(&self) -> Option<&T> {
        self.upstream
            .as_ref()
            .map(TrackedUpstreamAssociation::target)
    }

    pub(crate) fn association(&self) -> Option<&A> {
        self.upstream
            .as_ref()
            .map(TrackedUpstreamAssociation::association)
    }

    pub(crate) fn insert(
        &mut self,
        target: T,
        association: A,
    ) -> Option<TrackedUpstreamAssociation<T, A>> {
        self.track(TrackedUpstreamAssociation::new(target, association))
    }

    pub(crate) fn track(
        &mut self,
        upstream: TrackedUpstreamAssociation<T, A>,
    ) -> Option<TrackedUpstreamAssociation<T, A>> {
        self.upstream.replace(upstream)
    }

    pub(crate) fn take(&mut self) -> Option<TrackedUpstreamAssociation<T, A>> {
        self.upstream.take()
    }
}

impl<T, A> Default for TrackedUpstreamAssociationState<T, A> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, A> TrackedUpstreamAssociationState<T, A>
where
    T: PartialEq,
{
    pub(crate) fn matches_target(&self, target: &T) -> bool {
        self.upstream
            .as_ref()
            .map(|upstream| upstream.target() == target)
            .unwrap_or(false)
    }
}

impl<T, A> UpstreamAssociationRuntime<T, A> {
    pub(crate) fn idle_deadline(&self) -> Option<TokioInstant> {
        self.idle_deadline
    }

    pub(crate) fn touch_idle(&mut self, timeout: Duration) {
        self.idle_deadline = Some(TokioInstant::now() + timeout);
    }

    fn take_upstream(&mut self) -> Option<TrackedUpstreamAssociation<T, A>> {
        self.idle_deadline = None;
        self.upstream.take()
    }
}

impl<T, A> Default for UpstreamAssociationRuntime<T, A> {
    fn default() -> Self {
        Self {
            upstream: TrackedUpstreamAssociationState::new(),
            idle_deadline: None,
        }
    }
}

impl<T, A> UpstreamAssociationRuntime<T, A>
where
    T: UpstreamAssociationTarget,
{
    pub(crate) fn upstream_outbound_tag(&self) -> Option<&str> {
        self.upstream
            .target()
            .map(UpstreamAssociationTarget::outbound_tag)
    }
}

impl<T, A> UpstreamAssociationRuntime<T, A>
where
    T: UpstreamAssociationTarget,
    A: UpstreamAssociationTransport<T>,
{
    pub(crate) async fn send_packet(
        &mut self,
        proxy: &Proxy,
        inbound_tag: &str,
        association: T,
        session: &Session,
        payload: &[u8],
    ) -> Result<usize, EngineError> {
        self.ensure_association(proxy, inbound_tag, association, session.id)
            .await?;

        let association_ref = self
            .upstream
            .association()
            .expect("successful establish stores upstream association");

        match association_ref
            .send_packet(&session.target, session.port, payload)
            .await
        {
            Ok(sent) => {
                proxy.record_udp_upstream_packet_sent();
                self.idle_deadline = Some(TokioInstant::now() + proxy.udp_upstream_idle_timeout());
                Ok(sent)
            }
            Err(error) => {
                proxy.record_udp_upstream_send_failure();
                self.drop_after_send_error(inbound_tag, &error);
                Err(error)
            }
        }
    }

    async fn ensure_association(
        &mut self,
        proxy: &Proxy,
        inbound_tag: &str,
        association: T,
        session_id: u64,
    ) -> Result<(), EngineError> {
        let needs_new_association = !self.upstream.matches_target(&association);

        if !needs_new_association {
            proxy.record_udp_upstream_association_reused();
            let (outbound_tag, server, port) = association.log_parts();
            crate::logging::log_udp_upstream_association_reused(
                inbound_tag,
                outbound_tag,
                server,
                port,
            );
            return Ok(());
        }

        if let Some(a) = self.upstream.take() {
            let (_, association) = a.into_parts();
            association.close(UpstreamAssociationCloseReason::Closed);
            self.idle_deadline = None;
        }

        match A::establish(proxy, association.clone(), session_id).await {
            Ok(a) => {
                proxy.record_udp_upstream_association_created();
                self.idle_deadline = Some(TokioInstant::now() + proxy.udp_upstream_idle_timeout());
                let (outbound_tag, server, port) = association.log_parts();
                crate::logging::log_udp_upstream_association_created(
                    inbound_tag,
                    outbound_tag,
                    server,
                    port,
                    proxy.udp_upstream_idle_timeout(),
                );
                let _ = self.upstream.insert(association, a);
                Ok(())
            }
            Err(error) => {
                proxy.record_udp_upstream_association_failed();
                Err(error)
            }
        }
    }

    pub(crate) fn drop_after_send_error(&mut self, inbound_tag: &str, error: &EngineError) {
        if let Some(assoc) = self.upstream.take() {
            let (record, association) = assoc.into_parts();
            association.close(UpstreamAssociationCloseReason::Dropped);
            let (outbound_tag, server, port) = record.log_parts();
            crate::logging::log_udp_upstream_association_dropped(
                inbound_tag,
                outbound_tag,
                server,
                port,
                error,
            );
        }
        self.idle_deadline = None;
    }

    pub(crate) fn close_idle(&mut self) -> Option<T> {
        self.take_upstream().map(|association| {
            let (target, association) = association.into_parts();
            association.close(UpstreamAssociationCloseReason::IdleTimeout);
            target
        })
    }

    pub(crate) fn close_dropped(&mut self) -> Option<T> {
        self.take_upstream().map(|association| {
            let (target, association) = association.into_parts();
            association.close(UpstreamAssociationCloseReason::Dropped);
            target
        })
    }

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

    pub(crate) fn close_all_upstreams(&mut self) {
        if let Some(association) = self.upstream.take() {
            let (_, association) = association.into_parts();
            association.close(UpstreamAssociationCloseReason::Closed);
        }
        self.idle_deadline = None;
    }
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

#[cfg(test)]
mod tests;
