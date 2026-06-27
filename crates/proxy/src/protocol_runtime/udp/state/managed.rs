use crate::protocol_runtime::udp::flows::{
    ManagedDatagramFlow, ManagedRelayStreamFlow, ManagedStreamPacketFlow,
};
use crate::protocol_runtime::udp::{FlowFailure, ProtocolUdpFlowSnapshot};
use crate::protocol_runtime::vless_udp::model::{
    VlessUdpRelayFinalHopStart, VlessUdpRelayTwoStream, VlessUdpStartFlow,
};
#[cfg(feature = "vmess")]
use crate::protocol_runtime::vmess_udp::model::{VmessUdpRelayFlowStart, VmessUdpStartFlow};
use crate::runtime::udp_flow::packet_path::ChainTask;
use tokio::task::JoinSet;
use zero_core::Address;
use zero_engine::EngineError;

use crate::runtime::udp_flow::sessions::UdpFlowSnapshot;
use crate::runtime::Proxy;

pub(in crate::protocol_runtime::udp) mod cached;
mod datagram;
pub(in crate::protocol_runtime::udp) mod model;
mod stream;

pub(crate) use cached::ManagedCachedHandlers;
use cached::ManagedCachedState;
use datagram::ManagedDatagramState;
pub(crate) use model::{
    ManagedCachedFlowSender, ManagedDatagramFlowHandler, ManagedStreamFlowHandler,
};
use stream::ManagedStreamState;

pub(crate) struct ManagedUdpHandlers {
    pub(crate) cached: ManagedCachedHandlers,
    pub(crate) datagram: Vec<Box<dyn ManagedDatagramFlowHandler>>,
    pub(crate) stream: Vec<Box<dyn ManagedStreamFlowHandler>>,
}

pub(in crate::protocol_runtime::udp) struct ManagedProtocolUdpState {
    cached: ManagedCachedState,
    datagram: ManagedDatagramState,
    stream: ManagedStreamState,
}

impl ManagedProtocolUdpState {
    pub(super) fn new(handlers: ManagedUdpHandlers) -> Self {
        Self {
            cached: ManagedCachedState::new(handlers.cached),
            datagram: ManagedDatagramState::new(handlers.datagram),
            stream: ManagedStreamState::new(handlers.stream),
        }
    }

    pub(in crate::protocol_runtime::udp) async fn send_existing_vless(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        proxy: &Proxy,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<Option<u64>, EngineError> {
        self.cached
            .send_existing(chain_tasks, proxy, target, port, payload)
            .await
    }

    #[cfg(feature = "vmess")]
    pub(in crate::protocol_runtime::udp) async fn send_existing_vmess(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        proxy: &Proxy,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<Option<u64>, EngineError> {
        self.cached
            .send_existing(chain_tasks, proxy, target, port, payload)
            .await
    }

    pub(in crate::protocol_runtime::udp) async fn start_vless_flow(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        flow: VlessUdpStartFlow<'_>,
    ) -> Result<(), EngineError> {
        self.cached
            .start_vless_flow(chain_tasks, flow)
            .await
            .expect("registered VLESS cached UDP handler")
    }

    pub(in crate::protocol_runtime::udp) async fn start_vless_relay_two_stream(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        flow: VlessUdpRelayTwoStream<'_>,
    ) -> Result<(), EngineError> {
        self.cached
            .start_vless_relay_two_stream(chain_tasks, flow)
            .await
            .expect("registered VLESS cached UDP handler")
    }

    pub(in crate::protocol_runtime::udp) async fn start_vless_relay_final_hop(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        flow: VlessUdpRelayFinalHopStart<'_>,
    ) -> Result<(), EngineError> {
        self.cached
            .start_vless_relay_final_hop(chain_tasks, flow)
            .await
            .expect("registered VLESS cached UDP handler")
    }

    #[cfg(feature = "vmess")]
    pub(in crate::protocol_runtime::udp) async fn start_vmess_flow(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        flow: VmessUdpStartFlow<'_>,
    ) -> Result<(), EngineError> {
        self.cached
            .start_vmess_flow(chain_tasks, flow)
            .await
            .expect("registered VMess cached UDP handler")
    }

    #[cfg(feature = "vmess")]
    pub(in crate::protocol_runtime::udp) async fn start_vmess_relay_flow(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        flow: VmessUdpRelayFlowStart<'_>,
    ) -> Result<(), EngineError> {
        self.cached
            .start_vmess_relay_flow(chain_tasks, flow)
            .await
            .expect("registered VMess cached UDP handler")
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
