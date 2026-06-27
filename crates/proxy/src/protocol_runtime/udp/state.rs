use std::time::Duration;

use std::collections::HashMap;
use tokio::time::Instant as TokioInstant;

use zero_engine::EngineError;

use super::flows::{ManagedDatagramFlow, ManagedRelayStreamFlow, ManagedStreamPacketFlow};
use super::{
    FlowFailure, ManagedUdpFlowKind, ManagedUdpFlowRequest, ProtocolUdpFlowResume,
    ProtocolUdpFlowSnapshot,
};
use crate::runtime::udp_flow::outbound::ManagedUdpFlowRef;

use cached::CachedProtocolUdpState;
pub(crate) use cached::CachedUdpHandlers;
use managed::ManagedProtocolUdpState;
pub(crate) use managed::{
    ManagedCachedFlowSender, ManagedDatagramFlowHandler, ManagedExistingSend, ManagedRelaySend,
    ManagedStreamFlowHandler, ManagedUdpHandlers,
};
use upstream::UpstreamAssociationState;
pub(crate) use upstream::{UpstreamAssociationHandler, UpstreamUdpHandlers};

mod cached;
mod forward;
pub(in crate::protocol_runtime::udp) mod managed;
mod upstream;

pub(crate) struct ProtocolUpstreamAssociationView<'a> {
    pub(crate) outbound_tag: &'a str,
}

pub(crate) struct ClosedProtocolUpstreamAssociation {
    pub(crate) outbound_tag: String,
    pub(crate) server: String,
    pub(crate) port: u16,
}

pub(crate) struct ProtocolUdpState {
    cached: CachedProtocolUdpState,
    pub(super) managed: ManagedProtocolUdpState,
    upstream: UpstreamAssociationState,
    managed_flows: HashMap<ManagedUdpFlowRef, ProtocolUdpFlowSnapshot>,
    next_managed_flow_id: u64,
}

pub(crate) struct ProtocolUdpHandlers {
    pub(crate) cached: CachedUdpHandlers,
    pub(crate) managed: ManagedUdpHandlers,
    pub(crate) upstream: UpstreamUdpHandlers,
}

impl ProtocolUdpState {
    pub(crate) fn new(handlers: ProtocolUdpHandlers) -> Self {
        Self {
            cached: CachedProtocolUdpState::new(handlers.cached),
            managed: ManagedProtocolUdpState::new(handlers.managed),
            upstream: UpstreamAssociationState::new(handlers.upstream),
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

    pub(crate) async fn recv_upstream_packet(&self, buf: &mut [u8]) -> Result<usize, EngineError> {
        self.upstream.recv_upstream_packet(buf).await
    }

    pub(crate) fn upstream_association_view(&self) -> Option<ProtocolUpstreamAssociationView<'_>> {
        self.upstream
            .upstream_outbound_tag()
            .map(|outbound_tag| ProtocolUpstreamAssociationView { outbound_tag })
    }

    pub(crate) fn upstream_idle_deadline(&self) -> Option<TokioInstant> {
        self.upstream.upstream_idle_deadline()
    }

    pub(crate) fn touch_upstream_idle(&mut self, timeout: Duration) {
        self.upstream.touch_upstream_idle(timeout);
    }

    pub(crate) fn drop_upstream_association(
        &mut self,
    ) -> Option<ClosedProtocolUpstreamAssociation> {
        self.upstream
            .drop_upstream_association()
            .map(closed_protocol_upstream_association)
    }

    pub(crate) fn close_idle_upstream(&mut self) -> Option<ClosedProtocolUpstreamAssociation> {
        self.upstream
            .close_idle_upstream()
            .map(closed_protocol_upstream_association)
    }

    pub(crate) fn close_all_upstreams(mut self) {
        self.upstream.close_all_upstreams();
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
                self.upstream
                    .start_upstream_flow(inbound_tag, request)
                    .await
            }
        }
    }
}

fn closed_protocol_upstream_association(
    (outbound_tag, server, port): (String, String, u16),
) -> ClosedProtocolUpstreamAssociation {
    ClosedProtocolUpstreamAssociation {
        outbound_tag,
        server,
        port,
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
