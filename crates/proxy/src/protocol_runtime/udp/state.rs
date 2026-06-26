use std::time::Duration;

use tokio::time::Instant as TokioInstant;

use crate::protocol_runtime::socks5_udp::{
    ClosedSocks5UdpAssociation, Socks5UdpAssociationView, Socks5UdpRuntime,
};
use crate::protocol_runtime::vless_udp::VlessUdpOutboundManager;
#[cfg(feature = "vmess")]
use crate::protocol_runtime::vmess_udp::VmessUdpOutboundManager;
use zero_engine::EngineError;

use super::{
    FlowFailure, ManagedDatagramFlow, ManagedRelayStreamFlow, ManagedStreamPacketFlow,
    ManagedUdpFlowKind, ManagedUdpFlowRequest, ProtocolUdpFlowResume,
};

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

    pub(crate) async fn start_managed_udp_flow(
        &mut self,
        inbound_tag: &str,
        request: ManagedUdpFlowRequest<'_>,
    ) -> Result<usize, FlowFailure> {
        match (&request.resume, request.kind) {
            (ProtocolUdpFlowResume::Socks5(resume), ManagedUdpFlowKind::RelayStream) => {
                let Some(proxy) = request.proxy else {
                    return Err(managed_flow_mismatch(
                        "udp_socks5_proxy",
                        request.server,
                        request.port,
                        "expected proxy context for SOCKS5 UDP flow",
                    ));
                };
                let packet = crate::protocol_runtime::socks5_udp::Socks5UdpPacketSend {
                    proxy,
                    tag: inbound_tag,
                    server: request.server,
                    port: request.port,
                    resume: ProtocolUdpFlowResume::Socks5(resume.clone()),
                    session: request.session,
                    payload: request.payload,
                };
                self.socks5
                    .send_packet(packet, inbound_tag)
                    .await
                    .map_err(|error| FlowFailure {
                        stage: "udp_upstream_send",
                        error,
                        upstream: Some((request.server.to_string(), request.port)),
                    })
            }
            #[cfg(feature = "shadowsocks")]
            (ProtocolUdpFlowResume::Shadowsocks(_), ManagedUdpFlowKind::Datagram) => {
                self.start_managed_datagram_flow(
                    request.chain_tasks,
                    ManagedDatagramFlow {
                        proxy: request.proxy,
                        session: request.session,
                        server: request.server,
                        port: request.port,
                        resume: request.resume,
                        payload: request.payload,
                    },
                )
                .await
            }
            #[cfg(feature = "hysteria2")]
            (ProtocolUdpFlowResume::Hysteria2(_), ManagedUdpFlowKind::Datagram) => {
                self.start_managed_datagram_flow(
                    request.chain_tasks,
                    ManagedDatagramFlow {
                        proxy: request.proxy,
                        session: request.session,
                        server: request.server,
                        port: request.port,
                        resume: request.resume,
                        payload: request.payload,
                    },
                )
                .await
            }
            #[cfg(feature = "trojan")]
            (ProtocolUdpFlowResume::Trojan(_), ManagedUdpFlowKind::StreamPacket) => {
                let Some(proxy) = request.proxy else {
                    return Err(managed_flow_mismatch(
                        "udp_trojan_proxy",
                        request.server,
                        request.port,
                        "expected proxy context for Trojan UDP flow",
                    ));
                };
                self.start_trojan_stream_packet_flow(ManagedStreamPacketFlow {
                    chain_tasks: request.chain_tasks,
                    proxy,
                    session: request.session,
                    server: request.server,
                    port: request.port,
                    resume: request.resume,
                    payload: request.payload,
                })
                .await
            }
            #[cfg(feature = "trojan")]
            (ProtocolUdpFlowResume::Trojan(_), ManagedUdpFlowKind::RelayStream) => {
                let Some(carrier) = request.carrier else {
                    return Err(managed_flow_mismatch(
                        "udp_trojan_carrier",
                        request.server,
                        request.port,
                        "expected relay carrier for Trojan UDP flow",
                    ));
                };
                self.start_trojan_relay_stream_flow(ManagedRelayStreamFlow {
                    chain_tasks: request.chain_tasks,
                    proxy: request.proxy,
                    session: request.session,
                    carrier,
                    tls_server_name: request.tls_server_name,
                    server: request.server,
                    port: request.port,
                    resume: request.resume,
                    payload: request.payload,
                })
                .await
            }
            #[cfg(feature = "mieru")]
            (ProtocolUdpFlowResume::Mieru(_), ManagedUdpFlowKind::StreamPacket) => {
                let Some(proxy) = request.proxy else {
                    return Err(managed_flow_mismatch(
                        "udp_mieru_proxy",
                        request.server,
                        request.port,
                        "expected proxy context for Mieru UDP flow",
                    ));
                };
                self.start_mieru_stream_packet_flow(ManagedStreamPacketFlow {
                    chain_tasks: request.chain_tasks,
                    proxy,
                    session: request.session,
                    server: request.server,
                    port: request.port,
                    resume: request.resume,
                    payload: request.payload,
                })
                .await
            }
            #[cfg(feature = "mieru")]
            (ProtocolUdpFlowResume::Mieru(_), ManagedUdpFlowKind::RelayStream) => {
                let Some(carrier) = request.carrier else {
                    return Err(managed_flow_mismatch(
                        "udp_mieru_carrier",
                        request.server,
                        request.port,
                        "expected relay carrier for Mieru UDP flow",
                    ));
                };
                self.start_mieru_relay_stream_flow(ManagedRelayStreamFlow {
                    chain_tasks: request.chain_tasks,
                    proxy: request.proxy,
                    session: request.session,
                    carrier,
                    tls_server_name: request.tls_server_name,
                    server: request.server,
                    port: request.port,
                    resume: request.resume,
                    payload: request.payload,
                })
                .await
            }
            _ => Err(managed_flow_mismatch(
                "udp_managed_flow_resume",
                request.server,
                request.port,
                "managed UDP flow kind does not match protocol resume",
            )),
        }
    }
}

fn managed_flow_mismatch(
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
