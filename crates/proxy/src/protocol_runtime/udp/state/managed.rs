#[cfg(feature = "hysteria2")]
use crate::protocol_runtime::udp::h2_manager::H2ChainManager;
#[cfg(feature = "mieru")]
use crate::protocol_runtime::udp::mieru_manager::MieruChainManager;
#[cfg(feature = "shadowsocks")]
use crate::protocol_runtime::udp::ss_manager::SsChainManager;
#[cfg(feature = "trojan")]
use crate::protocol_runtime::udp::trojan_manager::TrojanChainManager;
use crate::protocol_runtime::udp::{
    FlowFailure, ManagedDatagramFlow, ManagedRelayStreamFlow, ManagedStreamPacketFlow,
    ProtocolUdpFlowSnapshot,
};
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

use crate::runtime::udp_flow::sessions::UdpFlowSnapshot;
use crate::runtime::Proxy;

pub(in crate::protocol_runtime::udp) mod model;

use model::{ManagedExistingSend, ManagedRelaySend};

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
    async fn send_shadowsocks_existing(
        &mut self,
        request: ManagedExistingSend<'_>,
    ) -> Result<usize, FlowFailure> {
        self.shadowsocks.send_managed_existing(request).await
    }

    #[cfg(feature = "hysteria2")]
    async fn send_hysteria2_existing(
        &mut self,
        request: ManagedExistingSend<'_>,
    ) -> Result<usize, FlowFailure> {
        self.hysteria2.send_managed_existing(request).await
    }

    #[cfg(feature = "trojan")]
    async fn send_trojan_existing(
        &mut self,
        request: ManagedExistingSend<'_>,
    ) -> Result<usize, FlowFailure> {
        self.trojan.send_managed_existing(request).await
    }

    #[cfg(feature = "trojan")]
    async fn send_trojan_relay_existing(
        &mut self,
        request: ManagedRelaySend<'_>,
    ) -> Result<usize, FlowFailure> {
        self.trojan.send_managed_relay_existing(request).await
    }

    #[cfg(feature = "mieru")]
    async fn send_mieru_existing(
        &mut self,
        request: ManagedExistingSend<'_>,
    ) -> Result<usize, FlowFailure> {
        self.mieru.send_managed_existing(request).await
    }

    #[cfg(feature = "mieru")]
    async fn send_mieru_relay_existing(
        &mut self,
        request: ManagedRelaySend<'_>,
    ) -> Result<usize, FlowFailure> {
        self.mieru.send_managed_relay_existing(request).await
    }

    pub(in crate::protocol_runtime::udp) async fn start_datagram_flow(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        flow: ManagedDatagramFlow<'_>,
    ) -> Result<usize, FlowFailure> {
        #[cfg(feature = "shadowsocks")]
        if self.shadowsocks.supports_managed_existing(&flow.resume) {
            return self
                .send_shadowsocks_existing(ManagedExistingSend::datagram(chain_tasks, &flow))
                .await;
        }
        #[cfg(feature = "hysteria2")]
        if self.hysteria2.supports_managed_existing(&flow.resume) {
            return self
                .send_hysteria2_existing(ManagedExistingSend::datagram(chain_tasks, &flow))
                .await;
        }
        Err(flow_mismatch(
            "udp_managed_datagram_resume",
            flow.server,
            flow.port,
            "expected managed datagram UDP flow resume",
        ))
    }

    pub(in crate::protocol_runtime::udp) async fn start_stream_packet_flow(
        &mut self,
        request: ManagedStreamPacketFlow<'_>,
    ) -> Result<usize, FlowFailure> {
        #[cfg(feature = "trojan")]
        if self.trojan.supports_managed_existing(&request.resume) {
            return self
                .send_trojan_existing(ManagedExistingSend::stream_packet(request))
                .await;
        }
        #[cfg(feature = "mieru")]
        if self.mieru.supports_managed_existing(&request.resume) {
            return self
                .send_mieru_existing(ManagedExistingSend::stream_packet(request))
                .await;
        }
        Err(flow_mismatch(
            "udp_stream_packet_resume",
            request.server,
            request.port,
            "expected stream-packet UDP flow resume",
        ))
    }

    pub(in crate::protocol_runtime::udp) async fn start_relay_stream_flow(
        &mut self,
        request: ManagedRelayStreamFlow<'_>,
    ) -> Result<usize, FlowFailure> {
        #[cfg(feature = "trojan")]
        if self.trojan.supports_managed_relay_existing(&request.resume) {
            return self
                .send_trojan_relay_existing(ManagedRelaySend::relay_stream(request))
                .await;
        }
        #[cfg(feature = "mieru")]
        if self.mieru.supports_managed_relay_existing(&request.resume) {
            return self
                .send_mieru_relay_existing(ManagedRelaySend::relay_stream(request))
                .await;
        }
        Err(flow_mismatch(
            "udp_relay_stream_resume",
            request.server,
            request.port,
            "expected relay-stream UDP flow resume",
        ))
    }

    pub(in crate::protocol_runtime::udp) async fn forward_existing_flow(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        proxy: &Proxy,
        flow: &UdpFlowSnapshot,
        snapshot: &ProtocolUdpFlowSnapshot,
        payload: &[u8],
    ) -> Option<Result<usize, FlowFailure>> {
        let upstream = flow
            .outbound
            .upstream()
            .expect("protocol flow should have upstream");
        let resume = snapshot.resume();
        #[cfg(feature = "shadowsocks")]
        if self.shadowsocks.supports_managed_existing(resume) {
            return Some(
                self.send_shadowsocks_existing(ManagedExistingSend::forwarded(
                    chain_tasks,
                    proxy,
                    flow,
                    resume.clone(),
                    upstream.server,
                    upstream.port,
                    payload,
                ))
                .await,
            );
        }
        #[cfg(feature = "hysteria2")]
        if self.hysteria2.supports_managed_existing(resume) {
            return Some(
                self.send_hysteria2_existing(ManagedExistingSend::forwarded(
                    chain_tasks,
                    proxy,
                    flow,
                    resume.clone(),
                    upstream.server,
                    upstream.port,
                    payload,
                ))
                .await,
            );
        }
        #[cfg(feature = "trojan")]
        if self.trojan.supports_managed_existing(resume) {
            return Some(
                self.send_trojan_existing(ManagedExistingSend::forwarded(
                    chain_tasks,
                    proxy,
                    flow,
                    resume.clone(),
                    upstream.server,
                    upstream.port,
                    payload,
                ))
                .await,
            );
        }
        #[cfg(feature = "mieru")]
        if self.mieru.supports_managed_existing(resume) {
            return Some(
                self.send_mieru_existing(ManagedExistingSend::forwarded(
                    chain_tasks,
                    proxy,
                    flow,
                    resume.clone(),
                    upstream.server,
                    upstream.port,
                    payload,
                ))
                .await,
            );
        }
        None
    }
}

fn flow_mismatch(
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
