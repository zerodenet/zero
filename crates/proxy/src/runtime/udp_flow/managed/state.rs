use super::{
    ManagedDatagramFlow, ManagedRelayStreamFlow, ManagedStreamPacketFlow, ManagedUdpFlowResume,
    ManagedUdpFlowSnapshot,
};
use crate::runtime::udp_dispatch::FlowFailure;
use crate::runtime::udp_flow::managed::ManagedStreamFlowSender;
use crate::runtime::udp_flow::outbound::ManagedUdpFlowRef;
use crate::runtime::udp_flow::packet_path::ChainTask;
use std::collections::HashMap;
use tokio::task::JoinSet;
use zero_engine::EngineError;

use crate::runtime::udp_flow::sessions::UdpFlowSnapshot;
use crate::runtime::Proxy;

use super::datagram::ManagedDatagramState;
use super::model::{ManagedDatagramFlowHandler, ManagedStreamFlowHandler};
use super::stream::ManagedStreamState;

pub(crate) struct ManagedUdpHandlers {
    pub(crate) datagram: Vec<Box<dyn ManagedDatagramFlowHandler>>,
    pub(crate) stream: Vec<Box<dyn ManagedStreamFlowHandler>>,
}

pub(crate) struct ManagedProtocolUdpState {
    datagram: ManagedDatagramState,
    stream: ManagedStreamState,
    flows: HashMap<ManagedUdpFlowRef, ManagedUdpFlowSnapshot>,
    next_flow_id: u64,
}

impl ManagedProtocolUdpState {
    pub(crate) fn new(handlers: ManagedUdpHandlers) -> Self {
        Self {
            datagram: ManagedDatagramState::new(handlers.datagram),
            stream: ManagedStreamState::new(handlers.stream),
            flows: HashMap::new(),
            next_flow_id: 1,
        }
    }

    pub(crate) fn register_flow(&mut self, resume: ManagedUdpFlowResume) -> ManagedUdpFlowRef {
        let flow_ref = self.next_flow_ref();
        self.flows
            .insert(flow_ref, ManagedUdpFlowSnapshot::managed(resume));
        flow_ref
    }

    pub(crate) fn register_stream_sender(
        &mut self,
        sender: Box<dyn ManagedStreamFlowSender>,
    ) -> ManagedUdpFlowRef {
        let flow_ref = self.next_flow_ref();
        self.stream.register_sender(flow_ref, sender);
        flow_ref
    }

    pub(crate) fn flow_snapshot(
        &self,
        flow_ref: ManagedUdpFlowRef,
    ) -> Option<ManagedUdpFlowSnapshot> {
        self.flows.get(&flow_ref).cloned()
    }

    pub(crate) fn flow_resume(&self, flow_ref: ManagedUdpFlowRef) -> Option<ManagedUdpFlowResume> {
        self.flow_snapshot(flow_ref)
            .map(|snapshot| snapshot.resume().clone())
    }

    fn next_flow_ref(&mut self) -> ManagedUdpFlowRef {
        let flow_ref = ManagedUdpFlowRef::new(self.next_flow_id);
        self.next_flow_id += 1;
        flow_ref
    }

    pub(crate) async fn start_datagram_flow(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        flow: ManagedDatagramFlow<'_>,
    ) -> Result<usize, FlowFailure> {
        let server = flow.server;
        let port = flow.port;
        self.datagram
            .start_datagram_flow(chain_tasks, flow)
            .await
            .ok_or_else(|| {
                flow_mismatch(
                    "udp_managed_datagram_resume",
                    server,
                    port,
                    "expected managed datagram UDP flow resume",
                )
            })?
    }

    pub(crate) async fn start_stream_packet_flow(
        &mut self,
        request: ManagedStreamPacketFlow<'_>,
    ) -> Result<usize, FlowFailure> {
        self.stream.start_stream_packet_flow(request).await
    }

    pub(crate) async fn start_relay_stream_flow(
        &mut self,
        request: ManagedRelayStreamFlow<'_>,
    ) -> Result<usize, FlowFailure> {
        self.stream.start_relay_stream_flow(request).await
    }

    pub(crate) async fn forward_registered_stream_sender(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        proxy: &Proxy,
        flow_ref: ManagedUdpFlowRef,
        flow: &UdpFlowSnapshot,
        payload: &[u8],
    ) -> Option<Result<usize, FlowFailure>> {
        self.stream
            .forward_registered_sender(chain_tasks, proxy, flow_ref, flow, payload)
            .await
    }

    pub(crate) async fn forward_existing_flow(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        proxy: &Proxy,
        flow: &UdpFlowSnapshot,
        payload: &[u8],
        is_upstream_resume: impl FnOnce(&ManagedUdpFlowResume) -> bool,
    ) -> Result<Option<usize>, FlowFailure> {
        let Some(flow_ref) = flow.outbound.managed_flow() else {
            return Err(managed_forward_unavailable(
                "udp_protocol_forward",
                "direct, relay, and packet-path flows are handled outside managed UDP state",
            ));
        };

        if let Some(result) = self
            .forward_registered_stream_sender(chain_tasks, proxy, flow_ref, flow, payload)
            .await
        {
            return result.map(Some);
        }

        let Some(snapshot) = self.flow_snapshot(flow_ref) else {
            return Err(managed_forward_unavailable(
                "udp_protocol_forward",
                "managed UDP flow snapshot was dropped",
            ));
        };

        if is_upstream_resume(snapshot.resume()) {
            return Err(managed_forward_unavailable(
                "udp_protocol_forward",
                "upstream association flows are handled by generic UDP dispatch",
            ));
        }

        if let Some(result) = self
            .datagram
            .forward_existing_flow(chain_tasks, proxy, flow, &snapshot, payload)
            .await
        {
            return result.map(Some);
        }
        if let Some(result) = self
            .stream
            .forward_existing_flow(chain_tasks, proxy, flow, &snapshot, payload)
            .await
        {
            return result.map(Some);
        }

        Ok(None)
    }
}

fn managed_forward_unavailable(stage: &'static str, message: &'static str) -> FlowFailure {
    FlowFailure {
        stage,
        error: EngineError::Io(std::io::Error::other(message)),
        upstream: None,
    }
}

pub(super) fn flow_mismatch(
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
