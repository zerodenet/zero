use crate::runtime::udp_dispatch::FlowFailure;
use crate::runtime::udp_flow::managed::{
    ManagedUdpFlowKind, ManagedUdpFlowRequest, ManagedUdpFlowResume,
};
use crate::runtime::udp_flow::outbound::ManagedUdpFlowRef;
use crate::runtime::udp_flow::outbound::UdpFlowOutbound;
use crate::runtime::udp_flow::packet_path::ChainTask;
use crate::runtime::udp_flow::protocol_state::CachedProtocolFlowSender;
use crate::runtime::udp_flow::sessions::UdpFlowSnapshot;
use crate::runtime::Proxy;
use tokio::task::JoinSet;
use zero_core::Session;
use zero_engine::EngineError;

use super::{FlowStartResult, UdpDispatch};

#[derive(Clone, Copy)]
pub(crate) enum ManagedUdpOutboundKind {
    Relay,
    Datagram,
    StreamPacket,
}

pub(crate) struct ManagedProtocolUdpSend<'a> {
    pub(crate) proxy: Option<&'a Proxy>,
    pub(crate) tag: &'a str,
    pub(crate) session: &'a Session,
    pub(crate) carrier: Option<crate::transport::RelayCarrier>,
    pub(crate) tls_server_name: Option<&'a str>,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) resume: ManagedUdpFlowResume,
    pub(crate) payload: &'a [u8],
    pub(crate) kind: ManagedUdpFlowKind,
    pub(crate) outbound: ManagedUdpOutboundKind,
}

impl UdpDispatch {
    pub(crate) fn protocol_udp_chain_tasks(&mut self) -> &mut JoinSet<ChainTask> {
        &mut self.chain_tasks
    }

    pub(crate) fn register_cached_protocol_flow_sender(
        &mut self,
        sender: Box<dyn CachedProtocolFlowSender>,
    ) {
        self.protocol_state.register_cached_flow_sender(sender);
    }

    pub(crate) async fn start_managed_protocol_flow(
        &mut self,
        mut request: ManagedUdpFlowRequest<'_>,
    ) -> Result<usize, FlowFailure> {
        request.chain_tasks = Some(&mut self.chain_tasks);
        self.protocol_state
            .start_managed_udp_flow(&self.inbound_tag, request)
            .await
    }

    pub(crate) fn register_managed_protocol_flow(
        &mut self,
        resume: ManagedUdpFlowResume,
    ) -> ManagedUdpFlowRef {
        self.protocol_state.register_managed_flow(resume)
    }

    pub(crate) fn managed_protocol_flow_resume(
        &self,
        flow_ref: ManagedUdpFlowRef,
    ) -> Option<ManagedUdpFlowResume> {
        self.protocol_state.managed_flow_resume(flow_ref)
    }

    pub(crate) async fn send_managed_protocol_udp(
        &mut self,
        request: ManagedProtocolUdpSend<'_>,
    ) -> Result<usize, FlowFailure> {
        self.start_managed_protocol_flow(ManagedUdpFlowRequest {
            chain_tasks: None,
            proxy: request.proxy,
            kind: request.kind,
            outbound_tag: Some(request.tag),
            session: request.session,
            carrier: request.carrier,
            tls_server_name: request.tls_server_name,
            server: request.server,
            port: request.port,
            resume: request.resume,
            payload: request.payload,
        })
        .await
    }

    pub(crate) async fn start_tracked_managed_protocol_udp(
        &mut self,
        request: ManagedProtocolUdpSend<'_>,
    ) -> Result<FlowStartResult, FlowFailure> {
        let resume = request.resume.clone();
        let tag = request.tag.to_string();
        let server = request.server.to_string();
        let port = request.port;
        let outbound = request.outbound;
        let sent = self.send_managed_protocol_udp(request).await?;
        let managed = self.register_managed_protocol_flow(resume);
        let outbound = match outbound {
            ManagedUdpOutboundKind::Relay => UdpFlowOutbound::Relay {
                tag,
                server,
                port,
                managed,
            },
            ManagedUdpOutboundKind::Datagram => UdpFlowOutbound::Datagram {
                tag,
                server,
                port,
                managed,
            },
            ManagedUdpOutboundKind::StreamPacket => UdpFlowOutbound::StreamPacket {
                tag,
                server,
                port,
                managed,
            },
        };
        Ok(FlowStartResult::Flow {
            outbound: Box::new(outbound),
            tx_bytes: sent as u64,
        })
    }

    pub(super) async fn forward_managed_relay_flow(
        &mut self,
        proxy: &Proxy,
        flow: &UdpFlowSnapshot,
        managed: ManagedUdpFlowRef,
        payload: &[u8],
    ) -> Result<usize, EngineError> {
        let upstream = flow
            .outbound
            .upstream()
            .expect("relay flow should expose upstream endpoint");
        let resume = self
            .managed_protocol_flow_resume(managed)
            .expect("managed relay flow should have protocol resume");
        self.send_managed_protocol_udp(ManagedProtocolUdpSend {
            proxy: Some(proxy),
            tag: flow.outbound.tag(),
            session: &flow.session,
            carrier: None,
            tls_server_name: None,
            server: upstream.server,
            port: upstream.port,
            resume,
            payload,
            kind: ManagedUdpFlowKind::RelayStream,
            outbound: ManagedUdpOutboundKind::Relay,
        })
        .await
        .map_err(|failure| failure.error)
    }
}
