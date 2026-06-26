use crate::protocol_runtime::udp::{
    ManagedUdpFlowKind, ManagedUdpFlowRequest, ProtocolUdpFlowResume, ProtocolUdpFlowSnapshot,
};
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::runtime::udp_flow::outbound::UdpFlowOutbound;
use zero_core::Session;

pub(crate) struct Hysteria2DatagramSend<'a> {
    pub(crate) tag: &'a str,
    pub(crate) session: &'a Session,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) resume: ProtocolUdpFlowResume,
    pub(crate) payload: &'a [u8],
}

impl UdpDispatch {
    #[cfg(feature = "hysteria2")]
    pub(crate) async fn send_hysteria2_datagram(
        &mut self,
        request: Hysteria2DatagramSend<'_>,
    ) -> Result<usize, FlowFailure> {
        self.protocol_state
            .start_managed_udp_flow(
                &self.inbound_tag,
                ManagedUdpFlowRequest {
                    chain_tasks: &mut self.chain_tasks,
                    proxy: None,
                    kind: ManagedUdpFlowKind::Datagram,
                    outbound_tag: Some(request.tag),
                    session: request.session,
                    carrier: None,
                    tls_server_name: None,
                    server: request.server,
                    port: request.port,
                    resume: request.resume,
                    payload: request.payload,
                },
            )
            .await
    }

    #[cfg(feature = "hysteria2")]
    pub(crate) async fn start_hysteria2_datagram_flow(
        &mut self,
        request: Hysteria2DatagramSend<'_>,
    ) -> Result<FlowStartResult, FlowFailure> {
        let sent = self
            .send_hysteria2_datagram(Hysteria2DatagramSend {
                tag: request.tag,
                session: request.session,
                server: request.server,
                port: request.port,
                resume: request.resume.clone(),
                payload: request.payload,
            })
            .await?;
        Ok(FlowStartResult::Flow {
            outbound: Box::new(UdpFlowOutbound::Datagram {
                tag: request.tag.to_string(),
                server: request.server.to_string(),
                port: request.port,
                protocol: ProtocolUdpFlowSnapshot::managed(request.resume),
            }),
            tx_bytes: sent as u64,
        })
    }
}
