use crate::runtime::udp_flow::managed::ManagedUdpFlowResume;
#[cfg(any(
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
use crate::runtime::udp_flow::managed::{ManagedUdpHandlers, ManagedUdpState};
use crate::runtime::udp_flow::outbound::ManagedUdpFlowRef;
use std::collections::HashMap;

#[cfg(feature = "socks5")]
use super::super::upstream::UpstreamAssociationState;

#[cfg(feature = "socks5")]
pub(crate) struct RegisteredUpstreamAssociationView<'a> {
    pub(crate) outbound_tag: &'a str,
}

#[cfg(feature = "socks5")]
pub(crate) struct ClosedRegisteredUpstreamAssociation {
    pub(crate) outbound_tag: String,
    pub(crate) server: String,
    pub(crate) port: u16,
}

pub(crate) struct RegisteredUdpState {
    #[cfg(any(
        feature = "vless",
        feature = "hysteria2",
        feature = "shadowsocks",
        feature = "trojan",
        feature = "vmess",
        feature = "mieru"
    ))]
    pub(in crate::runtime::udp_flow::registered) managed: ManagedUdpState,
    #[cfg(feature = "socks5")]
    pub(in crate::runtime::udp_flow::registered) upstream: UpstreamAssociationState,
    pub(in crate::runtime::udp_flow::registered) managed_resumes:
        HashMap<ManagedUdpFlowRef, ManagedUdpFlowResume>,
    pub(in crate::runtime::udp_flow::registered) next_managed_flow_id: u64,
}

pub(crate) struct RegisteredUdpHandlers {
    #[cfg(any(
        feature = "vless",
        feature = "hysteria2",
        feature = "shadowsocks",
        feature = "trojan",
        feature = "vmess",
        feature = "mieru"
    ))]
    pub(crate) managed: ManagedUdpHandlers,
    #[cfg(feature = "socks5")]
    pub(crate) upstream: super::super::upstream::UpstreamUdpHandlers,
}
