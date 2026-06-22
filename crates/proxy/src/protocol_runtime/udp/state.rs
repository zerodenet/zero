use crate::protocol_runtime::vless_udp::VlessUdpOutboundManager;
#[cfg(feature = "vmess")]
use crate::protocol_runtime::vmess_udp::VmessUdpOutboundManager;
use tokio::task::JoinSet;
use zero_core::Address;
use zero_engine::{EngineError, ResolvedLeafOutbound};

#[cfg(feature = "shadowsocks")]
use super::packet_path_traits::{UdpFlowContext, UdpPacketRef};
use super::ChainTask;
use super::FlowFailure;
#[cfg(feature = "hysteria2")]
use super::H2ChainManager;
#[cfg(feature = "mieru")]
use super::MieruChainManager;
#[cfg(feature = "trojan")]
use super::TrojanChainManager;
#[cfg(feature = "shadowsocks")]
use super::{PacketPathManager, SsChainManager};
use crate::runtime::udp_flow::sessions::{UdpFlowOutbound, UdpFlowSnapshot};
use crate::runtime::Proxy;

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

    pub(crate) async fn send_existing_cached_flow(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        proxy: &Proxy,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<Option<u64>, EngineError> {
        if let Some(session_id) = self
            .vless
            .send_existing(chain_tasks, proxy, target, port, payload)
            .await?
        {
            return Ok(Some(session_id));
        }

        #[cfg(feature = "vmess")]
        if let Some(session_id) = self
            .vmess
            .send_existing(chain_tasks, proxy, target, port, payload)
            .await?
        {
            return Ok(Some(session_id));
        }

        Ok(None)
    }

    #[cfg(feature = "shadowsocks")]
    pub(crate) async fn send_packet_path_chain(
        &mut self,
        context: UdpFlowContext<'_>,
        proxy: &Proxy,
        carrier_leaf: &ResolvedLeafOutbound<'_>,
        datagram_leaf: &ResolvedLeafOutbound<'_>,
        packet: UdpPacketRef<'_>,
    ) -> Result<usize, FlowFailure> {
        self.packet_path
            .send(context, proxy, carrier_leaf, datagram_leaf, packet)
            .await
    }

    pub(crate) async fn forward_existing_protocol_flow(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        proxy: &Proxy,
        flow: &UdpFlowSnapshot,
        payload: &[u8],
    ) -> Result<usize, FlowFailure> {
        match &flow.outbound {
            #[cfg(feature = "shadowsocks")]
            UdpFlowOutbound::Shadowsocks {
                tag,
                server,
                port,
                password,
                cipher,
                packet_path_carrier,
            } => {
                if let Some(carrier) = packet_path_carrier {
                    self.packet_path
                        .send_with_snapshot(
                            UdpFlowContext {
                                chain_tasks,
                                session_id: flow.session.id,
                            },
                            carrier,
                            tag.as_str(),
                            server.as_str(),
                            *port,
                            password.as_str(),
                            cipher.as_str(),
                            UdpPacketRef {
                                target: &flow.session.target,
                                port: flow.session.port,
                                payload,
                            },
                        )
                        .await
                } else {
                    self.shadowsocks
                        .send_existing(super::SsSendExisting {
                            chain_tasks,
                            session_id: flow.session.id,
                            proxy,
                            server: server.as_str(),
                            port: *port,
                            password: password.as_str(),
                            cipher: cipher.as_str(),
                            target: &flow.session.target,
                            target_port: flow.session.port,
                            payload,
                        })
                        .await
                }
            }
            #[cfg(feature = "hysteria2")]
            UdpFlowOutbound::Hysteria2 {
                server,
                port,
                password,
                client_fingerprint,
                ..
            } => {
                self.hysteria2
                    .send_existing(super::H2SendExisting {
                        chain_tasks,
                        session_id: flow.session.id,
                        server: server.as_str(),
                        port: *port,
                        password: password.as_str(),
                        client_fingerprint: client_fingerprint.as_deref(),
                        target: &flow.session.target,
                        target_port: flow.session.port,
                        payload,
                    })
                    .await
            }
            #[cfg(feature = "trojan")]
            UdpFlowOutbound::Trojan {
                server,
                port,
                password,
                sni,
                insecure,
                client_fingerprint,
                relay_chain,
                ..
            } => {
                self.trojan
                    .send_existing(super::TrojanSendExisting {
                        chain_tasks,
                        session_id: flow.session.id,
                        proxy,
                        session: &flow.session,
                        server: server.as_str(),
                        port: *port,
                        password: password.as_str(),
                        sni: sni.as_deref(),
                        insecure: *insecure,
                        client_fingerprint: client_fingerprint.as_deref(),
                        relay_chain: *relay_chain,
                        target: &flow.session.target,
                        target_port: flow.session.port,
                        payload,
                    })
                    .await
            }
            #[cfg(feature = "mieru")]
            UdpFlowOutbound::Mieru {
                server,
                port,
                username,
                password,
                relay_chain,
                ..
            } => {
                self.mieru
                    .send_existing(
                        chain_tasks,
                        flow.session.id,
                        proxy,
                        &flow.session,
                        server.as_str(),
                        *port,
                        username.as_str(),
                        password.as_str(),
                        *relay_chain,
                        &flow.session.target,
                        flow.session.port,
                        payload,
                    )
                    .await
            }
            UdpFlowOutbound::Direct { .. } | UdpFlowOutbound::Socks5 { .. } => Err(FlowFailure {
                stage: "udp_protocol_forward",
                error: EngineError::Io(std::io::Error::other(
                    "direct and socks5 flows are handled by generic UDP dispatch",
                )),
                upstream: None,
            }),
        }
    }
}
