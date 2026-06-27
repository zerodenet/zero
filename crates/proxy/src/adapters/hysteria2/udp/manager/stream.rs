use crate::outbound::hysteria2::Hysteria2Connector;
use crate::runtime::orchestration::OutboundEndpoint;
use crate::runtime::udp_flow::packet_path::UdpPacketRef;
use std::sync::Arc;
use zero_engine::EngineError;

pub(super) struct PacketStream {
    pub(super) session: hysteria2::Hysteria2UdpFlowSession,
}

pub(super) async fn establish(
    endpoint: OutboundEndpoint<'_>,
    initial_packet: UdpPacketRef<'_>,
    resume: hysteria2::Hysteria2UdpFlowResume,
) -> Result<PacketStream, EngineError> {
    let conn = Arc::new(
        Hysteria2Connector::from_udp_profile(
            endpoint.server,
            endpoint.port,
            resume.connector_profile(),
        )
        .connect_raw()
        .await?,
    );
    let session = hysteria2::start_udp_flow_with_initial_packet(
        conn,
        initial_packet.target,
        initial_packet.port,
        initial_packet.payload,
        resume,
    );

    Ok(PacketStream { session })
}
