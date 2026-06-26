use super::super::state::ProtocolUdpState;
use super::super::{
    FlowFailure, ManagedRelayStreamFlow, ManagedStreamPacketFlow, ProtocolUdpFlowResume,
};

impl ProtocolUdpState {
    pub(crate) async fn start_managed_stream_packet_flow(
        &mut self,
        request: ManagedStreamPacketFlow<'_>,
    ) -> Result<usize, FlowFailure> {
        match &request.resume {
            #[cfg(feature = "trojan")]
            ProtocolUdpFlowResume::Trojan(_) => self.start_trojan_stream_packet_flow(request).await,
            #[cfg(feature = "mieru")]
            ProtocolUdpFlowResume::Mieru(_) => self.start_mieru_stream_packet_flow(request).await,
            _ => Err(resume_mismatch(
                "udp_stream_packet_resume",
                request.server,
                request.port,
                "expected stream-packet UDP flow resume",
            )),
        }
    }

    pub(crate) async fn start_managed_relay_stream_flow(
        &mut self,
        request: ManagedRelayStreamFlow<'_>,
    ) -> Result<usize, FlowFailure> {
        match &request.resume {
            #[cfg(feature = "trojan")]
            ProtocolUdpFlowResume::Trojan(_) => self.start_trojan_relay_stream_flow(request).await,
            #[cfg(feature = "mieru")]
            ProtocolUdpFlowResume::Mieru(_) => self.start_mieru_relay_stream_flow(request).await,
            _ => Err(resume_mismatch(
                "udp_relay_stream_resume",
                request.server,
                request.port,
                "expected relay-stream UDP flow resume",
            )),
        }
    }
}

fn resume_mismatch(
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
