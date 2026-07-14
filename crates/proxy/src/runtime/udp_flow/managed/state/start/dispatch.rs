use super::super::error::flow_mismatch;
use super::super::model::ManagedUdpState;
#[cfg(any(feature = "hysteria2", feature = "shadowsocks"))]
use crate::runtime::udp_flow::managed::flow::ManagedDatagramFlow;
#[cfg(any(
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru"
))]
use crate::runtime::udp_flow::managed::flow::ManagedRelayStreamFlow;
#[cfg(any(
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru"
))]
use crate::runtime::udp_flow::managed::flow::ManagedStreamPacketFlow;
use crate::runtime::udp_flow::managed::flow::{ManagedUdpFlowKind, ManagedUdpFlowRequest};
use crate::runtime::udp_flow::result::FlowFailure;

impl ManagedUdpState {
    pub(crate) async fn start_flow(
        &mut self,
        request: ManagedUdpFlowRequest<'_>,
    ) -> Result<Option<usize>, FlowFailure> {
        match request.kind {
            #[cfg(any(feature = "hysteria2", feature = "shadowsocks"))]
            ManagedUdpFlowKind::Datagram => {
                let Some(chain_tasks) = request.chain_tasks else {
                    return Err(flow_mismatch(
                        "udp_managed_flow_chain_tasks",
                        request.server,
                        request.port,
                        "expected chain task context for managed UDP flow",
                    ));
                };
                self.start_datagram_flow(
                    chain_tasks,
                    ManagedDatagramFlow {
                        services: request.services,
                        session: request.session,
                        server: request.server,
                        port: request.port,
                        resume: request.resume,
                        payload: request.payload,
                    },
                )
                .await
                .map(Some)
            }
            #[cfg(any(
                feature = "vless",
                feature = "vmess",
                feature = "trojan",
                feature = "mieru"
            ))]
            ManagedUdpFlowKind::StreamPacket => {
                let Some(chain_tasks) = request.chain_tasks else {
                    return Err(flow_mismatch(
                        "udp_managed_flow_chain_tasks",
                        request.server,
                        request.port,
                        "expected chain task context for managed UDP flow",
                    ));
                };
                let Some(services) = request.services else {
                    return Err(flow_mismatch(
                        "udp_stream_packet_proxy",
                        request.server,
                        request.port,
                        "expected proxy context for stream-packet UDP flow",
                    ));
                };
                self.start_stream_packet_flow(ManagedStreamPacketFlow {
                    chain_tasks,
                    services,
                    session: request.session,
                    server: request.server,
                    port: request.port,
                    resume: request.resume,
                    payload: request.payload,
                })
                .await
                .map(Some)
            }
            #[cfg(any(
                feature = "vless",
                feature = "vmess",
                feature = "trojan",
                feature = "mieru"
            ))]
            ManagedUdpFlowKind::RelayStream => {
                let Some(carrier) = request.carrier else {
                    return Ok(None);
                };
                let Some(chain_tasks) = request.chain_tasks else {
                    return Err(flow_mismatch(
                        "udp_managed_flow_chain_tasks",
                        request.server,
                        request.port,
                        "expected chain task context for managed UDP flow",
                    ));
                };
                self.start_relay_stream_flow(ManagedRelayStreamFlow {
                    chain_tasks,
                    services: request.services,
                    session: request.session,
                    carrier,
                    tls_server_name: request.tls_server_name,
                    server: request.server,
                    port: request.port,
                    resume: request.resume,
                    payload: request.payload,
                })
                .await
                .map(Some)
            }
        }
    }
}
