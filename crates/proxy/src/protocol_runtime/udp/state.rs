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

pub(crate) struct ProtocolUdpState {
    pub(crate) vless: VlessUdpOutboundManager,
    #[cfg(feature = "vmess")]
    pub(crate) vmess: VmessUdpOutboundManager,
    #[cfg(feature = "shadowsocks")]
    pub(crate) shadowsocks: SsChainManager,
    #[cfg(feature = "shadowsocks")]
    pub(crate) packet_path: PacketPathManager,
    #[cfg(feature = "trojan")]
    pub(crate) trojan: TrojanChainManager,
    #[cfg(feature = "mieru")]
    pub(crate) mieru: MieruChainManager,
    #[cfg(feature = "hysteria2")]
    pub(crate) hysteria2: H2ChainManager,
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
