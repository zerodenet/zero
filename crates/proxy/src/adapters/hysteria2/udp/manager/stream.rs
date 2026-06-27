use crate::runtime::orchestration::OutboundEndpoint;
use crate::runtime::udp_flow::packet_path::UdpPacketRef;
use zero_engine::EngineError;

pub(super) struct PacketStream {
    pub(super) session: hysteria2::Hysteria2UdpFlowSession,
}

pub(super) async fn establish(
    endpoint: OutboundEndpoint<'_>,
    initial_packet: UdpPacketRef<'_>,
    resume: hysteria2::Hysteria2UdpFlowResume,
) -> Result<PacketStream, EngineError> {
    let session =
        crate::outbound::hysteria2::establish_udp_flow_session(endpoint, initial_packet, resume)
            .await?;

    Ok(PacketStream { session })
}
