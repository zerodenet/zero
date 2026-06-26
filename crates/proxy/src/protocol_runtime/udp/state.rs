use std::time::Duration;

use std::collections::HashMap;
use tokio::time::Instant as TokioInstant;

use crate::protocol_runtime::socks5_udp::{
    ClosedSocks5UdpAssociation, Socks5UdpAssociationView, Socks5UdpRuntime,
};
use zero_engine::EngineError;

use super::{
    FlowFailure, ManagedDatagramFlow, ManagedRelayStreamFlow, ManagedStreamPacketFlow,
    ManagedUdpFlowKind, ManagedUdpFlowRequest, ProtocolUdpFlowResume, ProtocolUdpFlowSnapshot,
};
use crate::runtime::udp_flow::outbound::ManagedUdpFlowRef;

use managed::ManagedProtocolUdpState;

mod cached;
mod forward;
mod managed;

pub(crate) struct ProtocolUdpState {
    pub(super) socks5: Socks5UdpRuntime,
    pub(super) managed: ManagedProtocolUdpState,
    managed_flows: HashMap<ManagedUdpFlowRef, ProtocolUdpFlowSnapshot>,
    next_managed_flow_id: u64,
}

impl ProtocolUdpState {
    pub(crate) fn new() -> Self {
        Self {
            socks5: Socks5UdpRuntime::default(),
            managed: ManagedProtocolUdpState::new(),
            managed_flows: HashMap::new(),
            next_managed_flow_id: 1,
        }
    }

    pub(crate) fn register_managed_flow(
        &mut self,
        resume: ProtocolUdpFlowResume,
    ) -> ManagedUdpFlowRef {
        let flow_ref = ManagedUdpFlowRef::new(self.next_managed_flow_id);
        self.next_managed_flow_id += 1;
        self.managed_flows
            .insert(flow_ref, ProtocolUdpFlowSnapshot::managed(resume));
        flow_ref
    }

    pub(super) fn managed_flow_snapshot(
        &self,
        flow_ref: ManagedUdpFlowRef,
    ) -> Option<ProtocolUdpFlowSnapshot> {
        self.managed_flows.get(&flow_ref).cloned()
    }

    pub(crate) fn managed_flow_resume(
        &self,
        flow_ref: ManagedUdpFlowRef,
    ) -> Option<ProtocolUdpFlowResume> {
        self.managed_flow_snapshot(flow_ref)
            .map(|snapshot| snapshot.resume().clone())
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
        match request.kind {
            ManagedUdpFlowKind::Datagram => {
                let Some(chain_tasks) = request.chain_tasks else {
                    return Err(managed_flow_mismatch(
                        "udp_managed_flow_chain_tasks",
                        request.server,
                        request.port,
                        "expected chain task context for managed UDP flow",
                    ));
                };
                self.start_managed_datagram_flow(
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
            }
            ManagedUdpFlowKind::StreamPacket => {
                let Some(chain_tasks) = request.chain_tasks else {
                    return Err(managed_flow_mismatch(
                        "udp_managed_flow_chain_tasks",
                        request.server,
                        request.port,
                        "expected chain task context for managed UDP flow",
                    ));
                };
                let Some(proxy) = request.proxy else {
                    return Err(managed_flow_mismatch(
                        "udp_stream_packet_proxy",
                        request.server,
                        request.port,
                        "expected proxy context for stream-packet UDP flow",
                    ));
                };
                self.start_managed_stream_packet_flow(ManagedStreamPacketFlow {
                    chain_tasks,
                    proxy,
                    session: request.session,
                    server: request.server,
                    port: request.port,
                    resume: request.resume,
                    payload: request.payload,
                })
                .await
            }
            ManagedUdpFlowKind::RelayStream => {
                if let Some(carrier) = request.carrier {
                    let Some(chain_tasks) = request.chain_tasks else {
                        return Err(managed_flow_mismatch(
                            "udp_managed_flow_chain_tasks",
                            request.server,
                            request.port,
                            "expected chain task context for managed UDP flow",
                        ));
                    };
                    return self
                        .start_managed_relay_stream_flow(ManagedRelayStreamFlow {
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
                        .await;
                }
                self.start_socks5_relay_flow(inbound_tag, request).await
            }
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
