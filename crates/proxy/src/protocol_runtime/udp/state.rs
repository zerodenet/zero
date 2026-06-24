use std::time::Duration;

use tokio::time::Instant as TokioInstant;

use crate::protocol_runtime::socks5_udp::{
    ClosedSocks5UdpAssociation, Socks5UdpAssociationView, Socks5UdpPacketSend, Socks5UdpRuntime,
};
use crate::protocol_runtime::vless_udp::VlessUdpOutboundManager;
#[cfg(feature = "vmess")]
use crate::protocol_runtime::vmess_udp::VmessUdpOutboundManager;
use zero_engine::EngineError;

#[cfg(feature = "hysteria2")]
use super::h2_manager::H2ChainManager;
#[cfg(feature = "mieru")]
use super::mieru_manager::MieruChainManager;
#[cfg(feature = "shadowsocks")]
use super::ss_manager::SsChainManager;
#[cfg(feature = "trojan")]
use super::trojan_manager::TrojanChainManager;
#[cfg(feature = "shadowsocks")]
use super::PacketPathManager;

mod cached;
mod forward;
#[cfg(feature = "shadowsocks")]
mod packet_path;

pub(crate) struct ProtocolUdpState {
    pub(super) socks5: Socks5UdpRuntime,
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
            socks5: Socks5UdpRuntime::default(),
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

    pub(crate) async fn send_socks5_packet(
        &mut self,
        request: Socks5UdpPacketSend<'_>,
        inbound_tag: &str,
    ) -> Result<usize, EngineError> {
        self.socks5.send_packet(request, inbound_tag).await
    }

    pub(crate) fn socks5_runtime(&self) -> &Socks5UdpRuntime {
        &self.socks5
    }

    pub(crate) fn socks5_upstream_view(&self) -> Option<Socks5UdpAssociationView<'_>> {
        self.socks5.upstream_view()
    }

    pub(crate) fn socks5_idle_deadline(&self) -> Option<TokioInstant> {
        self.socks5.idle_deadline()
    }

    pub(crate) fn touch_socks5_idle(&mut self, timeout: Duration) {
        self.socks5.touch_idle(timeout);
    }

    pub(crate) fn drop_socks5_upstream(&mut self) -> Option<ClosedSocks5UdpAssociation> {
        self.socks5.close_dropped()
    }

    pub(crate) fn close_socks5_idle(&mut self) -> Option<ClosedSocks5UdpAssociation> {
        self.socks5.close_idle()
    }

    pub(crate) fn close_socks5_all(self) {
        self.socks5.close_all();
    }
}
