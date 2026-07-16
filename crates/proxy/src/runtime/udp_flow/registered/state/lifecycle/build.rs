use super::super::model::{RegisteredUdpHandlers, RegisteredUdpState};
#[cfg(any(
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
use crate::runtime::udp_flow::managed::ManagedUdpState;
#[cfg(feature = "socks5")]
use crate::runtime::udp_flow::registered::upstream::UpstreamAssociationState;

impl RegisteredUdpState {
    pub(crate) fn new(handlers: RegisteredUdpHandlers) -> Self {
        Self {
            #[cfg(any(
                feature = "vless",
                feature = "hysteria2",
                feature = "shadowsocks",
                feature = "trojan",
                feature = "vmess",
                feature = "mieru"
            ))]
            managed: ManagedUdpState::new(handlers.managed),
            #[cfg(feature = "socks5")]
            upstream: UpstreamAssociationState::new(handlers.upstream),
            managed_resumes: std::collections::HashMap::new(),
            next_managed_flow_id: 1,
        }
    }
}
