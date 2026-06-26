#[cfg(feature = "hysteria2")]
use crate::protocol_runtime::udp::h2_manager::model::H2SendExisting;
#[cfg(feature = "hysteria2")]
use crate::protocol_runtime::udp::h2_manager::H2ChainManager;
#[cfg(feature = "mieru")]
use crate::protocol_runtime::udp::mieru_manager::model::{MieruRelayExisting, MieruSendExisting};
#[cfg(feature = "mieru")]
use crate::protocol_runtime::udp::mieru_manager::MieruChainManager;
#[cfg(feature = "shadowsocks")]
use crate::protocol_runtime::udp::ss_manager::model::SsSendExisting;
#[cfg(feature = "shadowsocks")]
use crate::protocol_runtime::udp::ss_manager::SsChainManager;
#[cfg(feature = "trojan")]
use crate::protocol_runtime::udp::trojan_manager::model::{
    TrojanRelayExisting, TrojanSendExisting,
};
#[cfg(feature = "trojan")]
use crate::protocol_runtime::udp::trojan_manager::TrojanChainManager;
use crate::protocol_runtime::udp::FlowFailure;
use crate::protocol_runtime::vless_udp::model::{
    VlessUdpRelayFinalHopStart, VlessUdpRelayTwoStream, VlessUdpStartFlow,
};
use crate::protocol_runtime::vless_udp::VlessUdpOutboundManager;
#[cfg(feature = "vmess")]
use crate::protocol_runtime::vmess_udp::model::{VmessUdpRelayFlowStart, VmessUdpStartFlow};
#[cfg(feature = "vmess")]
use crate::protocol_runtime::vmess_udp::VmessUdpOutboundManager;
use crate::runtime::udp_flow::packet_path::ChainTask;
use tokio::task::JoinSet;
use zero_core::Address;
use zero_engine::EngineError;

use crate::runtime::Proxy;

pub(in crate::protocol_runtime::udp) struct ManagedProtocolUdpState {
    vless: VlessUdpOutboundManager,
    #[cfg(feature = "vmess")]
    vmess: VmessUdpOutboundManager,
    #[cfg(feature = "shadowsocks")]
    shadowsocks: SsChainManager,
    #[cfg(feature = "trojan")]
    trojan: TrojanChainManager,
    #[cfg(feature = "mieru")]
    mieru: MieruChainManager,
    #[cfg(feature = "hysteria2")]
    hysteria2: H2ChainManager,
}

impl ManagedProtocolUdpState {
    pub(super) fn new() -> Self {
        Self {
            vless: VlessUdpOutboundManager::new(),
            #[cfg(feature = "vmess")]
            vmess: VmessUdpOutboundManager::new(),
            #[cfg(feature = "shadowsocks")]
            shadowsocks: SsChainManager::new(),
            #[cfg(feature = "trojan")]
            trojan: TrojanChainManager::new(),
            #[cfg(feature = "mieru")]
            mieru: MieruChainManager::new(),
            #[cfg(feature = "hysteria2")]
            hysteria2: H2ChainManager::new(),
        }
    }

    pub(in crate::protocol_runtime::udp) async fn send_existing_vless(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        proxy: &Proxy,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<Option<u64>, EngineError> {
        self.vless
            .send_existing(chain_tasks, proxy, target, port, payload)
            .await
    }

    #[cfg(feature = "vmess")]
    pub(in crate::protocol_runtime::udp) async fn send_existing_vmess(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        proxy: &Proxy,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<Option<u64>, EngineError> {
        self.vmess
            .send_existing(chain_tasks, proxy, target, port, payload)
            .await
    }

    pub(in crate::protocol_runtime::udp) async fn start_vless_flow(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        flow: VlessUdpStartFlow<'_>,
    ) -> Result<(), EngineError> {
        self.vless.start_flow(chain_tasks, flow).await
    }

    pub(in crate::protocol_runtime::udp) async fn start_vless_relay_two_stream(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        flow: VlessUdpRelayTwoStream<'_>,
    ) -> Result<(), EngineError> {
        self.vless.start_relay_two_stream(chain_tasks, flow).await
    }

    pub(in crate::protocol_runtime::udp) async fn start_vless_relay_final_hop(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        flow: VlessUdpRelayFinalHopStart<'_>,
    ) -> Result<(), EngineError> {
        self.vless.start_relay_final_hop(chain_tasks, flow).await
    }

    #[cfg(feature = "vmess")]
    pub(in crate::protocol_runtime::udp) async fn start_vmess_flow(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        flow: VmessUdpStartFlow<'_>,
    ) -> Result<(), EngineError> {
        self.vmess.start_flow(chain_tasks, flow).await
    }

    #[cfg(feature = "vmess")]
    pub(in crate::protocol_runtime::udp) async fn start_vmess_relay_flow(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        flow: VmessUdpRelayFlowStart<'_>,
    ) -> Result<(), EngineError> {
        self.vmess.start_relay_flow(chain_tasks, flow).await
    }

    #[cfg(feature = "shadowsocks")]
    pub(in crate::protocol_runtime::udp) async fn send_shadowsocks_existing(
        &mut self,
        request: SsSendExisting<'_>,
    ) -> Result<usize, FlowFailure> {
        self.shadowsocks.send_existing(request).await
    }

    #[cfg(feature = "hysteria2")]
    pub(in crate::protocol_runtime::udp) async fn send_hysteria2_existing(
        &mut self,
        request: H2SendExisting<'_>,
    ) -> Result<usize, FlowFailure> {
        self.hysteria2.send_existing(request).await
    }

    #[cfg(feature = "trojan")]
    pub(in crate::protocol_runtime::udp) async fn send_trojan_existing(
        &mut self,
        request: TrojanSendExisting<'_>,
    ) -> Result<usize, FlowFailure> {
        self.trojan.send_existing(request).await
    }

    #[cfg(feature = "trojan")]
    pub(in crate::protocol_runtime::udp) async fn send_trojan_relay_existing(
        &mut self,
        request: TrojanRelayExisting<'_>,
    ) -> Result<usize, FlowFailure> {
        self.trojan.send_relay_existing(request).await
    }

    #[cfg(feature = "mieru")]
    pub(in crate::protocol_runtime::udp) async fn send_mieru_existing(
        &mut self,
        request: MieruSendExisting<'_>,
    ) -> Result<usize, FlowFailure> {
        self.mieru.send_existing(request).await
    }

    #[cfg(feature = "mieru")]
    pub(in crate::protocol_runtime::udp) async fn send_mieru_relay_existing(
        &mut self,
        request: MieruRelayExisting<'_>,
    ) -> Result<usize, FlowFailure> {
        self.mieru.send_relay_existing(request).await
    }
}
