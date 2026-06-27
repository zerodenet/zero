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
    pub(in crate::protocol_runtime::udp) chain_tasks: &'a mut JoinSet<ChainTask>,
    pub(in crate::protocol_runtime::udp) session_id: u64,
    pub(in crate::protocol_runtime::udp) proxy: Option<&'a Proxy>,
    pub(in crate::protocol_runtime::udp) session: &'a Session,
    pub(in crate::protocol_runtime::udp) server: &'a str,
    pub(in crate::protocol_runtime::udp) port: u16,
    pub(in crate::protocol_runtime::udp) resume: ProtocolUdpFlowResume,
    pub(in crate::protocol_runtime::udp) target: &'a Address,
    pub(in crate::protocol_runtime::udp) target_port: u16,
    pub(in crate::protocol_runtime::udp) payload: &'a [u8],
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
    pub(in crate::protocol_runtime::udp) chain_tasks: &'a mut JoinSet<ChainTask>,
    pub(in crate::protocol_runtime::udp) session_id: u64,
    pub(in crate::protocol_runtime::udp) stream: TcpRelayStream,
    pub(in crate::protocol_runtime::udp) tls_server_name: Option<&'a str>,
    pub(in crate::protocol_runtime::udp) proxy: Option<&'a Proxy>,
    pub(in crate::protocol_runtime::udp) session: &'a Session,
    pub(in crate::protocol_runtime::udp) server: &'a str,
    pub(in crate::protocol_runtime::udp) port: u16,
    pub(in crate::protocol_runtime::udp) resume: ProtocolUdpFlowResume,
    pub(in crate::protocol_runtime::udp) target: &'a Address,
    pub(in crate::protocol_runtime::udp) target_port: u16,
    pub(in crate::protocol_runtime::udp) payload: &'a [u8],
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
