#[cfg(all(
    feature = "socks5",
    any(
        feature = "vless",
        feature = "vmess",
        feature = "trojan",
        feature = "mieru"
    )
))]
use crate::runtime::udp_flow::managed::ManagedUdpFlowRequest;
use crate::runtime::udp_flow::managed::ManagedUdpFlowResume;
use crate::runtime::udp_flow::registered::upstream::UpstreamAssociationSend;
use crate::runtime::udp_flow::result::FlowFailure;

use super::super::model::RegisteredUdpState;

impl RegisteredUdpState {
    pub(crate) async fn start_upstream_udp_flow(
        &mut self,
        inbound_tag: &str,
        request: UpstreamAssociationSend<'_>,
    ) -> Result<usize, FlowFailure> {
        self.upstream
            .start_upstream_flow(inbound_tag, request)
            .await
    }

    pub(crate) fn handles_upstream_resume(&self, resume: &ManagedUdpFlowResume) -> bool {
        self.upstream.handles_resume(resume)
    }
}

#[cfg(all(
    feature = "socks5",
    any(
        feature = "vless",
        feature = "vmess",
        feature = "trojan",
        feature = "mieru"
    )
))]
pub(super) fn upstream_send(request: ManagedUdpFlowRequest<'_>) -> UpstreamAssociationSend<'_> {
    UpstreamAssociationSend {
        services: request.services,
        session: request.session,
        server: request.server,
        port: request.port,
        resume: request.resume,
        payload: request.payload,
    }
}
