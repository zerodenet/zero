use std::time::Duration;

use async_trait::async_trait;
use tokio::time::Instant as TokioInstant;
use zero_engine::EngineError;

use super::establish::{
    self, Socks5UdpAssociationEstablishRequest, Socks5UdpAssociationEstablisher,
};
use super::model::{
    BoxedSocks5UdpAssociation, ClosedSocks5UdpAssociation, Socks5UdpAssociationView,
    UpstreamAssociationCloseReason,
};
use super::send::{self, Socks5UdpSend};
use crate::runtime::udp_dispatch::FlowFailure;
use crate::runtime::udp_flow::managed::{ManagedUdpFlowRequest, ManagedUdpFlowResume};
use crate::runtime::udp_flow::registered::UpstreamAssociationHandler;
use crate::runtime::udp_flow::response::UpstreamUdpResponse;

pub(crate) struct Socks5UdpRuntime {
    pub(super) upstream: Option<BoxedSocks5UdpAssociation>,
    pub(super) idle_deadline: Option<TokioInstant>,
    establisher: Box<dyn Socks5UdpAssociationEstablisher>,
}

impl Default for Socks5UdpRuntime {
    fn default() -> Self {
        Self {
            upstream: None,
            idle_deadline: None,
            establisher: establish::default_establisher(),
        }
    }
}

impl Socks5UdpRuntime {
    pub(crate) fn handles_resume(&self, resume: &ManagedUdpFlowResume) -> bool {
        resume
            .as_ref::<socks5::udp::Socks5UdpFlowResume>()
            .is_some()
    }

    pub(crate) fn idle_deadline(&self) -> Option<TokioInstant> {
        self.idle_deadline
    }

    pub(crate) fn touch_idle(&mut self, timeout: Duration) {
        self.idle_deadline = Some(TokioInstant::now() + timeout);
    }

    fn take_upstream(&mut self) -> Option<BoxedSocks5UdpAssociation> {
        self.idle_deadline = None;
        self.upstream.take()
    }

    pub(crate) fn upstream_view(&self) -> Option<Socks5UdpAssociationView<'_>> {
        self.upstream
            .as_ref()
            .map(|upstream| Socks5UdpAssociationView {
                outbound_tag: upstream.outbound_tag(),
            })
    }

    async fn send(
        &mut self,
        request: Socks5UdpSend<'_>,
        inbound_tag: &str,
    ) -> Result<usize, EngineError> {
        send::send(request, inbound_tag, self).await
    }

    pub(super) async fn send_packet(
        &mut self,
        proxy: &crate::runtime::Proxy,
        inbound_tag: &str,
        association: socks5::udp::Socks5UdpAssociationTarget,
        session: &zero_core::Session,
        payload: &[u8],
    ) -> Result<usize, EngineError> {
        self.ensure_association(proxy, inbound_tag, association, session.id)
            .await?;

        let association_ref = self
            .upstream
            .as_ref()
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
        proxy: &crate::runtime::Proxy,
        inbound_tag: &str,
        association: socks5::udp::Socks5UdpAssociationTarget,
        session_id: u64,
    ) -> Result<(), EngineError> {
        let target = association.identity();
        let needs_new_association = self
            .upstream
            .as_ref()
            .map(|a| !a.identity().matches(&target))
            .unwrap_or(true);

        if !needs_new_association {
            proxy.record_udp_upstream_association_reused();
            crate::logging::log_udp_upstream_association_reused(
                inbound_tag,
                target.outbound_tag(),
                target.server(),
                target.port(),
            );
            return Ok(());
        }

        if let Some(a) = self.upstream.take() {
            a.close(UpstreamAssociationCloseReason::Closed);
            self.idle_deadline = None;
        }

        match self
            .establisher
            .establish_boxed(Socks5UdpAssociationEstablishRequest {
                proxy,
                target: association,
                session_id,
            })
            .await
        {
            Ok(a) => {
                proxy.record_udp_upstream_association_created();
                self.idle_deadline = Some(TokioInstant::now() + proxy.udp_upstream_idle_timeout());
                crate::logging::log_udp_upstream_association_created(
                    inbound_tag,
                    target.outbound_tag(),
                    target.server(),
                    target.port(),
                    proxy.udp_upstream_idle_timeout(),
                );
                self.upstream = Some(a);
                Ok(())
            }
            Err(error) => {
                proxy.record_udp_upstream_association_failed();
                Err(error)
            }
        }
    }

    pub(super) fn drop_after_send_error(&mut self, inbound_tag: &str, error: &EngineError) {
        if let Some(assoc) = self.upstream.take() {
            let active = assoc.identity();
            assoc.close(UpstreamAssociationCloseReason::Dropped);
            crate::logging::log_udp_upstream_association_dropped(
                inbound_tag,
                active.outbound_tag(),
                active.server(),
                active.port(),
                error,
            );
        }
        self.idle_deadline = None;
    }

    pub(crate) async fn start_relay_flow(
        &mut self,
        inbound_tag: &str,
        request: ManagedUdpFlowRequest<'_>,
    ) -> Result<usize, FlowFailure> {
        let Some(proxy) = request.proxy else {
            return Err(socks5_flow_mismatch(
                "udp_socks5_proxy",
                request.server,
                request.port,
                "expected proxy context for SOCKS5 UDP flow",
            ));
        };
        let Some(outbound_tag) = request.outbound_tag else {
            return Err(socks5_flow_mismatch(
                "udp_socks5_outbound_tag",
                request.server,
                request.port,
                "expected outbound tag for SOCKS5 UDP flow",
            ));
        };

        self.send(
            Socks5UdpSend {
                proxy,
                tag: outbound_tag,
                server: request.server,
                port: request.port,
                resume: request.resume,
                session: request.session,
                payload: request.payload,
            },
            inbound_tag,
        )
        .await
        .map_err(|error| FlowFailure {
            stage: "udp_upstream_send",
            error,
            upstream: Some((request.server.to_string(), request.port)),
        })
    }

    pub(crate) fn close_idle(&mut self) -> Option<ClosedSocks5UdpAssociation> {
        self.take_upstream().map(|association| {
            let active = association.identity();
            association.close(UpstreamAssociationCloseReason::IdleTimeout);
            let (outbound_tag, server, port) = active.into_parts();
            ClosedSocks5UdpAssociation {
                outbound_tag,
                server,
                port,
            }
        })
    }

    pub(crate) fn close_dropped(&mut self) -> Option<ClosedSocks5UdpAssociation> {
        self.take_upstream().map(|association| {
            let active = association.identity();
            association.close(UpstreamAssociationCloseReason::Dropped);
            let (outbound_tag, server, port) = active.into_parts();
            ClosedSocks5UdpAssociation {
                outbound_tag,
                server,
                port,
            }
        })
    }

    pub(crate) async fn recv_upstream_response(
        &self,
        buf: &mut [u8],
    ) -> Result<UpstreamUdpResponse, EngineError> {
        match self.upstream.as_ref() {
            Some(association) => {
                let (target, port, payload) = association.recv_response_parts(buf).await?;
                Ok(UpstreamUdpResponse::new(target, port, payload))
            }
            None => std::future::pending::<Result<UpstreamUdpResponse, EngineError>>().await,
        }
    }
}

#[async_trait]
impl UpstreamAssociationHandler for Socks5UdpRuntime {
    fn supports_upstream_resume(&self, resume: &ManagedUdpFlowResume) -> bool {
        self.handles_resume(resume)
    }

    async fn send_upstream(
        &mut self,
        inbound_tag: &str,
        request: ManagedUdpFlowRequest<'_>,
    ) -> Result<usize, FlowFailure> {
        self.start_relay_flow(inbound_tag, request).await
    }

    async fn recv_upstream_response(
        &self,
        buf: &mut [u8],
    ) -> Result<UpstreamUdpResponse, EngineError> {
        self.recv_upstream_response(buf).await
    }

    fn upstream_outbound_tag(&self) -> Option<&str> {
        self.upstream_view()
            .map(|association| association.outbound_tag)
    }

    fn upstream_idle_deadline(&self) -> Option<TokioInstant> {
        self.idle_deadline()
    }

    fn touch_upstream_idle(&mut self, timeout: Duration) {
        self.touch_idle(timeout);
    }

    fn drop_upstream_association(&mut self) -> Option<(String, String, u16)> {
        self.close_dropped()
            .map(closed_protocol_upstream_association)
    }

    fn close_idle_upstream(&mut self) -> Option<(String, String, u16)> {
        self.close_idle().map(closed_protocol_upstream_association)
    }

    fn close_all_upstreams(&mut self) {
        if let Some(association) = self.upstream.take() {
            association.close(UpstreamAssociationCloseReason::Closed);
        }
        self.idle_deadline = None;
    }
}

fn closed_protocol_upstream_association(
    closed: ClosedSocks5UdpAssociation,
) -> (String, String, u16) {
    (closed.outbound_tag, closed.server, closed.port)
}

fn socks5_flow_mismatch(
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
