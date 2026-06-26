use super::super::state::ProtocolUdpState;
use super::super::{FlowFailure, ManagedUdpFlowRequest};

impl ProtocolUdpState {
    pub(crate) async fn start_socks5_relay_flow(
        &mut self,
        inbound_tag: &str,
        request: ManagedUdpFlowRequest<'_>,
    ) -> Result<usize, FlowFailure> {
        let Some(proxy) = request.proxy else {
            return Err(socks5_flow_mismatch(
                "udp_socks5_proxy",
                request.server,
                request.port,
                "expected proxy context for SOCKS5 UDP flow",
            ));
        };
        let packet = crate::protocol_runtime::socks5_udp::Socks5UdpPacketSend {
            proxy,
            tag: inbound_tag,
            server: request.server,
            port: request.port,
            resume: request.resume,
            session: request.session,
            payload: request.payload,
        };
        self.socks5
            .send_packet(packet, inbound_tag)
            .await
            .map_err(|error| FlowFailure {
                stage: "udp_upstream_send",
                error,
                upstream: Some((request.server.to_string(), request.port)),
            })
    }
}

fn socks5_flow_mismatch(
    stage: &'static str,
    server: &str,
    port: u16,
    message: &'static str,
) -> FlowFailure {
    FlowFailure {
        stage,
        error: zero_engine::EngineError::Io(std::io::Error::other(message)),
        upstream: Some((server.to_string(), port)),
    }
}
