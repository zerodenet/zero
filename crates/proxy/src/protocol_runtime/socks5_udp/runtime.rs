use std::time::Duration;

use tokio::time::Instant as TokioInstant;
use zero_core::Session;
use zero_engine::EngineError;

use super::active::ActiveUpstreamSocks5UdpAssociation;
use super::model::{
    ClosedSocks5UdpAssociation, Socks5UdpAssociationView, UpstreamAssociationCloseReason,
};
use super::send::{self, Socks5UdpSend};
use crate::protocol_runtime::udp::ProtocolUdpFlowResume;

#[derive(Default)]
pub(crate) struct Socks5UdpRuntime {
    pub(super) upstream: Option<ActiveUpstreamSocks5UdpAssociation>,
    pub(super) idle_deadline: Option<TokioInstant>,
}

impl Socks5UdpRuntime {
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

    pub(crate) async fn send_packet(
        &mut self,
        request: Socks5UdpPacketSend<'_>,
        inbound_tag: &str,
    ) -> Result<usize, EngineError> {
        self.send(
            Socks5UdpSend {
                proxy: request.proxy,
                tag: request.tag,
                server: request.server,
                port: request.port,
                resume: request.resume,
                session: request.session,
                payload: request.payload,
            },
            inbound_tag,
        )
        .await
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

    pub(crate) fn close_all(mut self) {
        if let Some(association) = self.upstream.take() {
            association.close(UpstreamAssociationCloseReason::Closed);
        }
        self.idle_deadline = None;
    }
}

pub(crate) struct Socks5UdpPacketSend<'a> {
    pub(crate) proxy: &'a crate::runtime::Proxy,
    pub(crate) tag: &'a str,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) resume: ProtocolUdpFlowResume,
    pub(crate) session: &'a Session,
    pub(crate) payload: &'a [u8],
}

pub(crate) async fn recv_upstream_packet(
    runtime: &Socks5UdpRuntime,
    buf: &mut [u8],
) -> Result<usize, EngineError> {
    match runtime.upstream.as_ref() {
        Some(association) => association.recv_packet(buf).await,
        None => std::future::pending::<Result<usize, EngineError>>().await,
    }
}
