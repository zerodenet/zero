use tokio::task::JoinSet;
use zero_core::{Address, Session};

use crate::protocol_runtime::udp::flows::{
    ManagedDatagramFlow, ManagedRelayStreamFlow, ManagedStreamPacketFlow,
};
use crate::protocol_runtime::udp::{FlowFailure, ProtocolUdpFlowResume};
use crate::runtime::udp_flow::packet_path::ChainTask;
use crate::runtime::udp_flow::sessions::UdpFlowSnapshot;
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;

pub(crate) struct ManagedExistingSend<'a> {
    pub(crate) chain_tasks: &'a mut JoinSet<ChainTask>,
    pub(crate) session_id: u64,
    pub(crate) proxy: Option<&'a Proxy>,
    pub(crate) session: &'a Session,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) resume: ProtocolUdpFlowResume,
    pub(crate) target: &'a Address,
    pub(crate) target_port: u16,
    pub(crate) payload: &'a [u8],
}

#[async_trait::async_trait]
pub(crate) trait ManagedDatagramFlowHandler: Send + Sync {
    fn supports_managed_existing(&self, resume: &ProtocolUdpFlowResume) -> bool;

    async fn send_managed_existing(
        &mut self,
        request: ManagedExistingSend<'_>,
    ) -> Result<usize, FlowFailure>;
}

#[async_trait::async_trait]
pub(crate) trait ManagedStreamFlowHandler: Send + Sync {
    fn supports_managed_existing(&self, resume: &ProtocolUdpFlowResume) -> bool;

    fn supports_managed_relay_existing(&self, resume: &ProtocolUdpFlowResume) -> bool;

    async fn send_managed_existing(
        &mut self,
        request: ManagedExistingSend<'_>,
    ) -> Result<usize, FlowFailure>;

    async fn send_managed_relay_existing(
        &mut self,
        request: ManagedRelaySend<'_>,
    ) -> Result<usize, FlowFailure>;
}

#[async_trait::async_trait]
pub(crate) trait ManagedCachedFlowSender: Send + Sync {
    async fn send_existing(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        proxy: &Proxy,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<Option<u64>, zero_engine::EngineError>;
}

impl<'a> ManagedExistingSend<'a> {
    pub(in crate::protocol_runtime::udp) fn datagram(
        chain_tasks: &'a mut JoinSet<ChainTask>,
        flow: &ManagedDatagramFlow<'a>,
    ) -> Self {
        Self {
            chain_tasks,
            session_id: flow.session.id,
            proxy: flow.proxy,
            session: flow.session,
            server: flow.server,
            port: flow.port,
            resume: flow.resume.clone(),
            target: &flow.session.target,
            target_port: flow.session.port,
            payload: flow.payload,
        }
    }

    pub(in crate::protocol_runtime::udp) fn stream_packet(
        request: ManagedStreamPacketFlow<'a>,
    ) -> Self {
        Self {
            chain_tasks: request.chain_tasks,
            session_id: request.session.id,
            proxy: Some(request.proxy),
            session: request.session,
            server: request.server,
            port: request.port,
            resume: request.resume,
            target: &request.session.target,
            target_port: request.session.port,
            payload: request.payload,
        }
    }

    pub(in crate::protocol_runtime::udp) fn forwarded(
        chain_tasks: &'a mut JoinSet<ChainTask>,
        proxy: &'a Proxy,
        flow: &'a UdpFlowSnapshot,
        resume: ProtocolUdpFlowResume,
        server: &'a str,
        port: u16,
        payload: &'a [u8],
    ) -> Self {
        Self {
            chain_tasks,
            session_id: flow.session.id,
            proxy: Some(proxy),
            session: &flow.session,
            server,
            port,
            resume,
            target: &flow.session.target,
            target_port: flow.session.port,
            payload,
        }
    }
}

pub(crate) struct ManagedRelaySend<'a> {
    pub(crate) chain_tasks: &'a mut JoinSet<ChainTask>,
    pub(crate) session_id: u64,
    pub(crate) stream: TcpRelayStream,
    pub(crate) tls_server_name: Option<&'a str>,
    pub(crate) proxy: Option<&'a Proxy>,
    pub(crate) session: &'a Session,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) resume: ProtocolUdpFlowResume,
    pub(crate) target: &'a Address,
    pub(crate) target_port: u16,
    pub(crate) payload: &'a [u8],
}

impl<'a> ManagedRelaySend<'a> {
    pub(in crate::protocol_runtime::udp) fn relay_stream(
        request: ManagedRelayStreamFlow<'a>,
    ) -> Self {
        Self {
            chain_tasks: request.chain_tasks,
            session_id: request.session.id,
            stream: request.carrier.stream,
            tls_server_name: request.tls_server_name,
            proxy: request.proxy,
            session: request.session,
            server: request.server,
            port: request.port,
            resume: request.resume,
            target: &request.session.target,
            target_port: request.session.port,
            payload: request.payload,
        }
    }
}
