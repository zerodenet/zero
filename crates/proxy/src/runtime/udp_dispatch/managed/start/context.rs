use crate::runtime::udp_dispatch::UdpDispatch;
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
use crate::runtime::udp_flow::state::UdpFlowStartContext;

impl UdpDispatch {
    pub(crate) fn flow_start_context(&mut self) -> UdpFlowStartContext<'_> {
        UdpFlowStartContext::new(&self.inbound_tag, &mut self.flow_state)
    }

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
        request: ManagedUdpFlowRequest<'_>,
    ) -> Result<usize, crate::runtime::udp_dispatch::FlowFailure> {
        self.flow_state
            .start_managed_flow(&self.inbound_tag, request)
            .await
    }

    #[cfg(any(feature = "socks5", feature = "hysteria2", feature = "shadowsocks"))]
    pub(crate) fn register_managed_flow(
        &mut self,
        resume: ManagedUdpFlowResume,
    ) -> ManagedUdpFlowRef {
        self.flow_state.register_managed_flow(resume)
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
        self.flow_state.managed_flow_resume(flow_ref)
    }
}
