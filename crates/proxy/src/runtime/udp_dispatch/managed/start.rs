use super::model::{
    ManagedDatagramStart, ManagedRelayStart, ManagedStreamPacketStart, ManagedUdpOutboundKind,
    ManagedUdpSend,
};
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::runtime::udp_flow::managed::{
    ManagedUdpFlowKind, ManagedUdpFlowRequest, ManagedUdpFlowResume,
};
use crate::runtime::udp_flow::outbound::{ManagedUdpFlowRef, UdpFlowOutbound};

impl UdpDispatch {
    pub(crate) async fn start_managed_flow(
        &mut self,
        request: ManagedUdpFlowRequest<'_>,
    ) -> Result<usize, FlowFailure> {
        self.flow_state
            .start_managed_flow(&self.inbound_tag, request)
            .await
    }

    pub(crate) fn register_managed_flow(
        &mut self,
        resume: ManagedUdpFlowResume,
    ) -> ManagedUdpFlowRef {
        self.flow_state.register_managed_flow(resume)
    }

    pub(crate) fn managed_flow_resume(
        &self,
        flow_ref: ManagedUdpFlowRef,
    ) -> Option<ManagedUdpFlowResume> {
        self.flow_state.managed_flow_resume(flow_ref)
    }

    pub(in crate::runtime::udp_dispatch::managed) async fn send_managed_udp(
        &mut self,
        request: ManagedUdpSend<'_>,
    ) -> Result<usize, FlowFailure> {
        self.start_managed_flow(ManagedUdpFlowRequest {
            chain_tasks: None,
            proxy: request.proxy,
            kind: request.kind,
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

    async fn start_tracked_managed_udp(
        &mut self,
        request: ManagedUdpSend<'_>,
    ) -> Result<FlowStartResult, FlowFailure> {
        let resume = request.resume.clone();
        let tag = request.tag.to_string();
        let server = request.server.to_string();
        let port = request.port;
        let outbound = request.outbound;
        let sent = self.send_managed_udp(request).await?;
        let managed = self.register_managed_flow(resume);
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

    pub(crate) async fn start_tracked_managed_datagram<T>(
        &mut self,
        request: ManagedDatagramStart<'_, T>,
    ) -> Result<FlowStartResult, FlowFailure>
    where
        T: std::any::Any + Send + Sync + std::fmt::Debug,
    {
        self.start_tracked_managed_udp(ManagedUdpSend {
            proxy: request.proxy,
            tag: request.tag,
            session: request.session,
            carrier: None,
            tls_server_name: None,
            server: request.server,
            port: request.port,
            resume: ManagedUdpFlowResume::new(request.resume),
            payload: request.payload,
            kind: ManagedUdpFlowKind::Datagram,
            outbound: ManagedUdpOutboundKind::Datagram,
        })
        .await
    }

    pub(crate) async fn start_tracked_managed_relay<T>(
        &mut self,
        request: ManagedRelayStart<'_, T>,
    ) -> Result<FlowStartResult, FlowFailure>
    where
        T: std::any::Any + Send + Sync + std::fmt::Debug,
    {
        self.start_tracked_managed_udp(ManagedUdpSend {
            proxy: request.proxy,
            tag: request.tag,
            session: request.session,
            carrier: request.carrier,
            tls_server_name: request.tls_server_name,
            server: request.server,
            port: request.port,
            resume: ManagedUdpFlowResume::new(request.resume),
            payload: request.payload,
            kind: ManagedUdpFlowKind::RelayStream,
            outbound: ManagedUdpOutboundKind::Relay,
        })
        .await
    }

    pub(crate) async fn start_tracked_managed_stream_packet<T>(
        &mut self,
        request: ManagedStreamPacketStart<'_, T>,
    ) -> Result<FlowStartResult, FlowFailure>
    where
        T: std::any::Any + Send + Sync + std::fmt::Debug,
    {
        self.start_tracked_managed_udp(ManagedUdpSend {
            proxy: request.proxy,
            tag: request.tag,
            session: request.session,
            carrier: request.carrier,
            tls_server_name: request.tls_server_name,
            server: request.server,
            port: request.port,
            resume: ManagedUdpFlowResume::new(request.resume),
            payload: request.payload,
            kind: if request.relay_chain {
                ManagedUdpFlowKind::RelayStream
            } else {
                ManagedUdpFlowKind::StreamPacket
            },
            outbound: ManagedUdpOutboundKind::StreamPacket,
        })
        .await
    }
}
