use tokio::task::JoinSet;
use zero_engine::EngineError;

use super::ProtocolUdpState;
use crate::protocol_runtime::udp::{ChainTask, FlowFailure};
#[cfg(feature = "hysteria2")]
mod hysteria2;
#[cfg(feature = "mieru")]
mod mieru;
#[cfg(feature = "shadowsocks")]
mod shadowsocks;
#[cfg(feature = "trojan")]
mod trojan;
use crate::runtime::udp_flow::outbound::UdpFlowOutbound;
use crate::runtime::udp_flow::sessions::UdpFlowSnapshot;
use crate::runtime::Proxy;

impl ProtocolUdpState {
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
                shadowsocks::forward(
                    self,
                    chain_tasks,
                    proxy,
                    flow,
                    shadowsocks::ExistingFlow {
                        tag,
                        server,
                        port: *port,
                        password,
                        cipher,
                        packet_path_carrier: packet_path_carrier.as_ref(),
                        payload,
                    },
                )
                .await
            }
            #[cfg(feature = "hysteria2")]
            UdpFlowOutbound::Hysteria2 {
                server,
                port,
                password,
                client_fingerprint,
                ..
            } => {
                hysteria2::forward(
                    self,
                    chain_tasks,
                    flow,
                    hysteria2::ExistingFlow {
                        server,
                        port: *port,
                        password,
                        client_fingerprint: client_fingerprint.as_deref(),
                        payload,
                    },
                )
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
                trojan::forward(
                    self,
                    chain_tasks,
                    proxy,
                    flow,
                    trojan::ExistingFlow {
                        server,
                        port: *port,
                        password,
                        sni: sni.as_deref(),
                        insecure: *insecure,
                        client_fingerprint: client_fingerprint.as_deref(),
                        relay_chain: *relay_chain,
                        payload,
                    },
                )
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
                mieru::forward(
                    self,
                    chain_tasks,
                    proxy,
                    flow,
                    mieru::ExistingFlow {
                        server,
                        port: *port,
                        username,
                        password,
                        relay_chain: *relay_chain,
                        payload,
                    },
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
