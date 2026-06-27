use crate::protocol_runtime::udp::flows::{
    ManagedDatagramFlow, ManagedRelayStreamFlow, ManagedStreamPacketFlow,
};
use crate::protocol_runtime::udp::{FlowFailure, ProtocolUdpFlowSnapshot};
use crate::runtime::udp_flow::packet_path::ChainTask;
use tokio::task::JoinSet;
use zero_engine::EngineError;

use crate::runtime::udp_flow::sessions::UdpFlowSnapshot;
use crate::runtime::Proxy;

mod datagram;
pub(in crate::protocol_runtime::udp) mod model;
mod stream;

use datagram::ManagedDatagramState;
pub(crate) use model::{
    ManagedCachedFlowSender, ManagedDatagramFlowHandler, ManagedStreamFlowHandler,
};
use stream::ManagedStreamState;

pub(crate) struct ManagedUdpHandlers {
    pub(crate) datagram: Vec<Box<dyn ManagedDatagramFlowHandler>>,
    pub(crate) stream: Vec<Box<dyn ManagedStreamFlowHandler>>,
}

pub(in crate::protocol_runtime::udp) struct ManagedProtocolUdpState {
    datagram: ManagedDatagramState,
    stream: ManagedStreamState,
}

impl ManagedProtocolUdpState {
    pub(super) fn new(handlers: ManagedUdpHandlers) -> Self {
        Self {
            datagram: ManagedDatagramState::new(handlers.datagram),
            stream: ManagedStreamState::new(handlers.stream),
        }
    }

    pub(in crate::protocol_runtime::udp) async fn start_datagram_flow(
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

    pub(in crate::protocol_runtime::udp) async fn start_stream_packet_flow(
        &mut self,
        request: ManagedStreamPacketFlow<'_>,
    ) -> Result<usize, FlowFailure> {
        self.stream.start_stream_packet_flow(request).await
    }

    pub(in crate::protocol_runtime::udp) async fn start_relay_stream_flow(
        &mut self,
        request: ManagedRelayStreamFlow<'_>,
    ) -> Result<usize, FlowFailure> {
        self.stream.start_relay_stream_flow(request).await
    }

    pub(in crate::protocol_runtime::udp) async fn forward_existing_flow(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        proxy: &Proxy,
        flow: &UdpFlowSnapshot,
        snapshot: &ProtocolUdpFlowSnapshot,
        payload: &[u8],
    ) -> Option<Result<usize, FlowFailure>> {
        if let Some(result) = self
            .datagram
            .forward_existing_flow(chain_tasks, proxy, flow, snapshot, payload)
            .await
        {
            return Some(result);
        }
        self.stream
            .forward_existing_flow(chain_tasks, proxy, flow, snapshot, payload)
            .await
    }
}

pub(in crate::protocol_runtime::udp::state::managed) fn flow_mismatch(
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
