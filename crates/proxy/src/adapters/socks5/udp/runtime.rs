use std::time::Duration;

use async_trait::async_trait;
use tokio::time::Instant as TokioInstant;
use zero_engine::EngineError;

use super::active::ActiveUpstreamSocks5UdpAssociation;
use crate::runtime::udp_dispatch::FlowFailure;
use crate::runtime::udp_flow::managed::{ManagedUdpFlowRequest, ManagedUdpFlowResume};
use crate::runtime::udp_flow::registered::{
    UpstreamAssociationHandler, UpstreamAssociationRuntime, UpstreamAssociationTarget,
};
use crate::runtime::udp_flow::response::UpstreamUdpResponse;

type Socks5UpstreamAssociationRuntime = UpstreamAssociationRuntime<
    socks5::udp::Socks5UdpAssociationTarget,
    ActiveUpstreamSocks5UdpAssociation,
>;

impl UpstreamAssociationTarget for socks5::udp::Socks5UdpAssociationTarget {
    fn outbound_tag(&self) -> &str {
        self.outbound_tag()
    }

    fn log_parts(&self) -> (&str, &str, u16) {
        self.log_parts()
    }
}

#[derive(Default)]
pub(crate) struct Socks5UdpRuntime {
    runtime: Socks5UpstreamAssociationRuntime,
}

impl Socks5UdpRuntime {
    pub(crate) fn handles_resume(&self, resume: &ManagedUdpFlowResume) -> bool {
        resume
            .as_ref::<socks5::udp::Socks5UdpFlowResume>()
            .is_some()
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

        let Some(resume) = request.resume.as_ref::<socks5::udp::Socks5UdpFlowResume>() else {
            return Err(socks5_flow_mismatch(
                "udp_socks5_resume",
                request.server,
                request.port,
                "expected SOCKS5 UDP flow resume",
            ));
        };
        let association = resume.association_target(
            outbound_tag.to_owned(),
            request.server.to_owned(),
            request.port,
        );

        match self
            .runtime
            .send_packet(
                proxy,
                inbound_tag,
                association,
                request.session,
                request.payload,
            )
            .await
        {
            Ok(sent) => Ok(sent),
            Err(error) => {
                proxy.record_udp_upstream_send_failure();
                Err(FlowFailure {
                    stage: "udp_upstream_send",
                    error,
                    upstream: Some((request.server.to_string(), request.port)),
                })
            }
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
        self.runtime
            .close_dropped()
            .map(socks5::udp::Socks5UdpAssociationTarget::into_log_parts)
    }

    fn close_idle_upstream(&mut self) -> Option<(String, String, u16)> {
        self.runtime
            .close_idle()
            .map(socks5::udp::Socks5UdpAssociationTarget::into_log_parts)
    }

    fn close_all_upstreams(&mut self) {
        self.runtime.close_all_upstreams();
    }
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
