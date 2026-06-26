use crate::protocol_runtime::udp::{ProtocolUdpFlowResume, ProtocolUdpFlowSnapshot};
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::runtime::udp_flow::outbound::UdpFlowOutbound;
use crate::runtime::Proxy;
use zero_core::Session;

pub(crate) struct MieruDatagramSend<'a> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) tag: &'a str,
    pub(crate) session: &'a Session,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) resume: ProtocolUdpFlowResume,
    pub(crate) payload: &'a [u8],
}

pub(crate) struct MieruRelaySend<'a> {
    pub(crate) tag: &'a str,
    pub(crate) session: &'a Session,
    pub(crate) carrier: crate::transport::RelayCarrier,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) resume: ProtocolUdpFlowResume,
    pub(crate) payload: &'a [u8],
}

impl UdpDispatch {
    pub(crate) async fn send_mieru_datagram(
        &mut self,
        request: MieruDatagramSend<'_>,
    ) -> Result<usize, FlowFailure> {
        self.protocol_state
            .start_mieru_stream_packet_flow(crate::protocol_runtime::udp::ManagedStreamPacketFlow {
                chain_tasks: &mut self.chain_tasks,
                proxy: request.proxy,
                session: request.session,
                server: request.server,
                port: request.port,
                resume: request.resume,
                payload: request.payload,
            })
            .await
    }

    pub(crate) async fn start_mieru_datagram_flow(
        &mut self,
        request: MieruDatagramSend<'_>,
    ) -> Result<FlowStartResult, FlowFailure> {
        let sent = self
            .send_mieru_datagram(MieruDatagramSend {
                proxy: request.proxy,
                tag: request.tag,
                session: request.session,
                server: request.server,
                port: request.port,
                resume: request.resume.clone(),
                payload: request.payload,
            })
            .await?;
        Ok(FlowStartResult::Flow {
            outbound: Box::new(UdpFlowOutbound::StreamPacket {
                tag: request.tag.to_string(),
                server: request.server.to_string(),
                port: request.port,
                protocol: ProtocolUdpFlowSnapshot::managed(request.resume),
            }),
            tx_bytes: sent as u64,
        })
    }

    pub(crate) async fn send_mieru_relay(
        &mut self,
        request: MieruRelaySend<'_>,
    ) -> Result<usize, FlowFailure> {
        self.protocol_state
            .start_mieru_udp_relay_flow(
                &mut self.chain_tasks,
                crate::protocol_runtime::udp::MieruUdpRelayFlow {
                    session: request.session,
                    carrier: request.carrier,
                    server: request.server,
                    port: request.port,
                    resume: request.resume,
                    payload: request.payload,
                },
            )
            .await
    }

    pub(crate) async fn start_mieru_relay_flow(
        &mut self,
        request: MieruRelaySend<'_>,
    ) -> Result<FlowStartResult, FlowFailure> {
        let sent = self
            .send_mieru_relay(MieruRelaySend {
                tag: request.tag,
                session: request.session,
                carrier: request.carrier,
                server: request.server,
                port: request.port,
                resume: request.resume.clone(),
                payload: request.payload,
            })
            .await?;
        Ok(FlowStartResult::Flow {
            outbound: Box::new(UdpFlowOutbound::StreamPacket {
                tag: request.tag.to_string(),
                server: request.server.to_string(),
                port: request.port,
                protocol: ProtocolUdpFlowSnapshot::managed(request.resume),
            }),
            tx_bytes: sent as u64,
        })
    }
}
