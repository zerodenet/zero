use super::super::error::flow_mismatch;
use super::super::model::ManagedUdpState;
use crate::runtime::udp_dispatch::FlowFailure;
use crate::runtime::udp_flow::managed::flow::{
    ManagedDatagramFlow, ManagedRelayStreamFlow, ManagedStreamPacketFlow, ManagedUdpFlowKind,
    ManagedUdpFlowRequest,
};

impl ManagedUdpState {
    pub(crate) async fn start_flow(
        &mut self,
        request: ManagedUdpFlowRequest<'_>,
    ) -> Result<Option<usize>, FlowFailure> {
        match request.kind {
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
                        proxy: request.proxy,
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
            ManagedUdpFlowKind::StreamPacket => {
                let Some(chain_tasks) = request.chain_tasks else {
                    return Err(flow_mismatch(
                        "udp_managed_flow_chain_tasks",
                        request.server,
                        request.port,
                        "expected chain task context for managed UDP flow",
                    ));
                };
                let Some(proxy) = request.proxy else {
                    return Err(flow_mismatch(
                        "udp_stream_packet_proxy",
                        request.server,
                        request.port,
                        "expected proxy context for stream-packet UDP flow",
                    ));
                };
                self.start_stream_packet_flow(ManagedStreamPacketFlow {
                    chain_tasks,
                    proxy,
                    session: request.session,
                    server: request.server,
                    port: request.port,
                    resume: request.resume,
                    payload: request.payload,
                })
                .await
                .map(Some)
            }
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
                .map(Some)
            }
        }
    }
}
