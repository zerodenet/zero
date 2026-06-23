use crate::protocol_runtime::vless_udp::VlessUdpOutboundManager;
#[cfg(feature = "vmess")]
use crate::protocol_runtime::vmess_udp::VmessUdpOutboundManager;

#[cfg(feature = "hysteria2")]
use super::H2ChainManager;
#[cfg(feature = "mieru")]
use super::MieruChainManager;
#[cfg(feature = "trojan")]
use super::TrojanChainManager;
#[cfg(feature = "shadowsocks")]
use super::{PacketPathManager, SsChainManager};

mod cached;
mod forward;
#[cfg(feature = "shadowsocks")]
mod packet_path;

pub(crate) struct ProtocolUdpState {
    pub(super) vless: VlessUdpOutboundManager,
    #[cfg(feature = "vmess")]
    pub(super) vmess: VmessUdpOutboundManager,
    #[cfg(feature = "shadowsocks")]
    pub(super) shadowsocks: SsChainManager,
    #[cfg(feature = "shadowsocks")]
    pub(super) packet_path: PacketPathManager,
    #[cfg(feature = "trojan")]
    pub(super) trojan: TrojanChainManager,
    #[cfg(feature = "mieru")]
    pub(super) mieru: MieruChainManager,
    #[cfg(feature = "hysteria2")]
    pub(super) hysteria2: H2ChainManager,
}

impl ProtocolUdpState {
    pub(crate) fn new() -> Self {
        Self {
            vless: VlessUdpOutboundManager::new(),
            #[cfg(feature = "vmess")]
            vmess: VmessUdpOutboundManager::new(),
            #[cfg(feature = "shadowsocks")]
            shadowsocks: SsChainManager::new(),
            #[cfg(feature = "shadowsocks")]
            packet_path: PacketPathManager::new(),
            #[cfg(feature = "trojan")]
            trojan: TrojanChainManager::new(),
            #[cfg(feature = "mieru")]
            mieru: MieruChainManager::new(),
            #[cfg(feature = "hysteria2")]
            hysteria2: H2ChainManager::new(),
        }
    }
}
