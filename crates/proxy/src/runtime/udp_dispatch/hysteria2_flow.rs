use zero_core::{Address, Error, Session};
use zero_traits::DatagramCodec;

use crate::protocol_runtime::udp::ProtocolUdpFlowSnapshot;
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::runtime::udp_flow::outbound::UdpFlowOutbound;

pub(crate) struct Hysteria2DatagramSend<'a> {
    pub(crate) tag: &'a str,
    pub(crate) session: &'a Session,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) resume: hysteria2::Hysteria2UdpFlowResume,
    pub(crate) codec: std::sync::Arc<dyn DatagramCodec<Address, Error = Error>>,
    pub(crate) payload: &'a [u8],
}

impl UdpDispatch {
    #[cfg(feature = "hysteria2")]
    pub(crate) async fn send_hysteria2_datagram(
        &mut self,
        request: Hysteria2DatagramSend<'_>,
    ) -> Result<usize, FlowFailure> {
        self.protocol_state
            .start_hysteria2_udp_flow(crate::protocol_runtime::udp::Hysteria2UdpFlowRequest {
                chain_tasks: &mut self.chain_tasks,
                session: request.session,
                server: request.server,
                port: request.port,
                password: request.resume.password(),
                client_fingerprint: request.resume.client_fingerprint(),
                codec: request.codec.clone(),
                payload: request.payload,
            })
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
                codec: request.codec.clone(),
                payload: request.payload,
            })
            .await?;
        Ok(FlowStartResult::Flow {
            outbound: Box::new(UdpFlowOutbound::Datagram {
                tag: request.tag.to_string(),
                server: request.server.to_string(),
                port: request.port,
                protocol: ProtocolUdpFlowSnapshot::hysteria2(request.resume),
            }),
            tx_bytes: sent as u64,
        })
    }
}
