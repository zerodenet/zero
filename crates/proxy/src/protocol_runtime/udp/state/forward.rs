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
use crate::protocol_runtime::udp::ProtocolUdpFlowSnapshot;
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
        let Some(snapshot) = flow.outbound.protocol_snapshot() else {
            return Err(FlowFailure {
                stage: "udp_protocol_forward",
                error: EngineError::Io(std::io::Error::other(
                    "direct and relay flows are handled by generic UDP dispatch",
                )),
                upstream: None,
            });
        };

        match snapshot {
            ProtocolUdpFlowSnapshot::Socks5 { .. } => Err(FlowFailure {
                stage: "udp_protocol_forward",
                error: EngineError::Io(std::io::Error::other(
                    "SOCKS5 relay flows are handled by generic UDP dispatch",
                )),
                upstream: None,
            }),
            #[cfg(feature = "shadowsocks")]
            ProtocolUdpFlowSnapshot::Shadowsocks {
                password,
                datagram_cache_key,
                cipher_kind,
                packet_path_carrier,
            } => {
                shadowsocks::forward(
                    self,
                    chain_tasks,
                    proxy,
                    flow,
                    shadowsocks::ExistingFlow {
                        tag: flow.outbound.tag(),
                        server: flow
                            .outbound
                            .upstream()
                            .expect("protocol flow should have upstream")
                            .server,
                        port: flow
                            .outbound
                            .upstream()
                            .expect("protocol flow should have upstream")
                            .port,
                        password,
                        datagram_cache_key,
                        cipher_kind: *cipher_kind,
                        packet_path_carrier: packet_path_carrier.as_ref(),
                        payload,
                    },
                )
                .await
            }
            #[cfg(feature = "hysteria2")]
            ProtocolUdpFlowSnapshot::Hysteria2 {
                password,
                client_fingerprint,
            } => {
                hysteria2::forward(
                    self,
                    chain_tasks,
                    flow,
                    hysteria2::ExistingFlow {
                        server: flow
                            .outbound
                            .upstream()
                            .expect("protocol flow should have upstream")
                            .server,
                        port: flow
                            .outbound
                            .upstream()
                            .expect("protocol flow should have upstream")
                            .port,
                        password,
                        client_fingerprint: client_fingerprint.as_deref(),
                        payload,
                    },
                )
                .await
            }
            #[cfg(feature = "trojan")]
            ProtocolUdpFlowSnapshot::Trojan {
                password,
                sni,
                insecure,
                client_fingerprint,
                relay_chain,
            } => {
                trojan::forward(
                    self,
                    chain_tasks,
                    proxy,
                    flow,
                    trojan::ExistingFlow {
                        server: flow
                            .outbound
                            .upstream()
                            .expect("protocol flow should have upstream")
                            .server,
                        port: flow
                            .outbound
                            .upstream()
                            .expect("protocol flow should have upstream")
                            .port,
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
            ProtocolUdpFlowSnapshot::Mieru {
                username,
                password,
                relay_chain,
            } => {
                mieru::forward(
                    self,
                    chain_tasks,
                    proxy,
                    flow,
                    mieru::ExistingFlow {
                        server: flow
                            .outbound
                            .upstream()
                            .expect("protocol flow should have upstream")
                            .server,
                        port: flow
                            .outbound
                            .upstream()
                            .expect("protocol flow should have upstream")
                            .port,
                        username,
                        password,
                        relay_chain: *relay_chain,
                        payload,
                    },
                )
                .await
            }
        }
    }
}
