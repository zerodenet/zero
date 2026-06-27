use std::time::Duration;

use async_trait::async_trait;
use tokio::time::Instant as TokioInstant;
use zero_engine::EngineError;

use super::active::ActiveUpstreamSocks5UdpAssociation;
use super::model::{
    ClosedSocks5UdpAssociation, Socks5UdpAssociationView, UpstreamAssociationCloseReason,
};
use super::send::{self, Socks5UdpSend};
use crate::protocol_runtime::udp::{
    FlowFailure, ManagedUdpFlowRequest, UpstreamAssociationHandler,
};
use crate::runtime::udp_flow::managed::ManagedUdpFlowResume;

#[derive(Default)]
pub(crate) struct Socks5UdpRuntime {
    pub(super) upstream: Option<ActiveUpstreamSocks5UdpAssociation>,
    pub(super) idle_deadline: Option<TokioInstant>,
}

impl Socks5UdpRuntime {
    pub(crate) fn handles_resume(&self, resume: &ManagedUdpFlowResume) -> bool {
        resume.as_ref::<socks5::Socks5UdpFlowResume>().is_some()
    }

    pub(crate) fn idle_deadline(&self) -> Option<TokioInstant> {
        self.idle_deadline
    }

    pub(crate) fn touch_idle(&mut self, timeout: Duration) {
        self.idle_deadline = Some(TokioInstant::now() + timeout);
    }

    fn take_upstream(&mut self) -> Option<ActiveUpstreamSocks5UdpAssociation> {
        self.idle_deadline = None;
        self.upstream.take()
    }

    pub(crate) fn upstream_view(&self) -> Option<Socks5UdpAssociationView<'_>> {
        self.upstream
            .as_ref()
            .map(|association| Socks5UdpAssociationView {
                outbound_tag: association.outbound_tag(),
            })
    }

    async fn send(
        &mut self,
        request: Socks5UdpSend<'_>,
        inbound_tag: &str,
    ) -> Result<usize, EngineError> {
        send::send(request, inbound_tag, self).await
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
            let outbound_tag = association.outbound_tag().to_owned();
            let (server, port) = association.upstream_endpoint();
            let server = server.to_owned();
            association.close(UpstreamAssociationCloseReason::IdleTimeout);
            ClosedSocks5UdpAssociation {
                outbound_tag,
                server,
                port,
            }
        })
    }

    pub(crate) fn close_dropped(&mut self) -> Option<ClosedSocks5UdpAssociation> {
        self.take_upstream().map(|association| {
            let outbound_tag = association.outbound_tag().to_owned();
            let (server, port) = association.upstream_endpoint();
            let server = server.to_owned();
            association.close(UpstreamAssociationCloseReason::Dropped);
            ClosedSocks5UdpAssociation {
                outbound_tag,
                server,
                port,
            }
        })
    }

    pub(crate) async fn recv_upstream_packet(&self, buf: &mut [u8]) -> Result<usize, EngineError> {
        match self.upstream.as_ref() {
            Some(association) => association.recv_packet(buf).await,
            None => std::future::pending::<Result<usize, EngineError>>().await,
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

    async fn recv_upstream_packet(&self, buf: &mut [u8]) -> Result<usize, EngineError> {
        self.recv_upstream_packet(buf).await
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
