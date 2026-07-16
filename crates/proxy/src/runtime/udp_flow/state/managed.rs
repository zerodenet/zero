#[cfg(any(
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
use crate::protocol_registry::UdpRuntimeServices;
#[cfg(any(
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
use crate::runtime::udp_flow::managed::ManagedExistingFlowForward;
#[cfg(any(
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
use crate::runtime::udp_flow::managed::ManagedUdpFlowRequest;
use crate::runtime::udp_flow::managed::ManagedUdpFlowResume;
use crate::runtime::udp_flow::outbound::ManagedUdpFlowRef;
#[cfg(feature = "socks5")]
use crate::runtime::udp_flow::registered::UpstreamAssociationSend;
use crate::runtime::udp_flow::result::FlowFailure;

use super::UdpFlowState;

impl UdpFlowState {
    #[cfg(any(
        feature = "vless",
        feature = "hysteria2",
        feature = "shadowsocks",
        feature = "trojan",
        feature = "vmess",
        feature = "mieru"
    ))]
    pub(crate) async fn start_managed_flow(
        &mut self,
        inbound_tag: &str,
        mut request: ManagedUdpFlowRequest<'_>,
    ) -> Result<usize, FlowFailure> {
        request.chain_tasks = Some(&mut self.chain_tasks);
        self.registered
            .start_managed_udp_flow(inbound_tag, request)
            .await
    }

    #[cfg(feature = "socks5")]
    pub(crate) async fn start_upstream_flow(
        &mut self,
        inbound_tag: &str,
        request: UpstreamAssociationSend<'_>,
    ) -> Result<usize, FlowFailure> {
        self.registered
            .start_upstream_udp_flow(inbound_tag, request)
            .await
    }

    #[cfg(feature = "socks5")]
    pub(crate) fn handles_upstream_resume(&self, resume: &ManagedUdpFlowResume) -> bool {
        self.registered.handles_upstream_resume(resume)
    }

    pub(crate) fn register_managed_flow(
        &mut self,
        resume: ManagedUdpFlowResume,
    ) -> ManagedUdpFlowRef {
        self.registered.register_managed_flow(resume)
    }

    #[cfg(any(
        feature = "socks5",
        feature = "vless",
        feature = "vmess",
        feature = "trojan",
        feature = "mieru"
    ))]
    pub(crate) fn managed_flow_resume(
        &self,
        flow_ref: ManagedUdpFlowRef,
    ) -> Option<ManagedUdpFlowResume> {
        self.registered.managed_flow_resume(flow_ref)
    }

    #[cfg(any(
        feature = "vless",
        feature = "hysteria2",
        feature = "shadowsocks",
        feature = "trojan",
        feature = "vmess",
        feature = "mieru"
    ))]
    pub(crate) async fn forward_existing_managed_flow(
        &mut self,
        services: UdpRuntimeServices,
        request: ManagedExistingFlowForward<'_>,
    ) -> Result<usize, FlowFailure> {
        self.registered
            .forward_existing_managed_flow(&mut self.chain_tasks, services, request)
            .await
    }
}
