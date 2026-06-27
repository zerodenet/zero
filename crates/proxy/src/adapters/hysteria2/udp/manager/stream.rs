use super::bridge;
use crate::outbound::hysteria2::Hysteria2Connector;
use crate::runtime::orchestration::OutboundEndpoint;
use crate::runtime::udp_flow::packet_path::UdpPacketRef;
use std::sync::Arc;
use zero_engine::EngineError;

pub(super) struct PacketStream {
    pub(super) sender: hysteria2::Hysteria2UdpFlowSender,
    pub(super) recv_tx: bridge::ResponseSender,
}

pub(super) async fn establish(
    endpoint: OutboundEndpoint<'_>,
    initial_packet: UdpPacketRef<'_>,
    resume: hysteria2::Hysteria2UdpFlowResume,
) -> Result<PacketStream, EngineError> {
    let connector_profile = resume.connector_profile();
    let flow_io = resume.flow_io();
    let conn = Arc::new(
        Hysteria2Connector::new(endpoint.server, endpoint.port, connector_profile.password())
            .with_fingerprint(connector_profile.client_fingerprint())
            .connect_raw()
            .await?,
    );
    let hysteria2::Hysteria2UdpFlowHandle { sender, responses } = hysteria2::spawn_udp_flow(
        conn,
        hysteria2::Hysteria2InitialUdpFlowPacket::from_parts(
            initial_packet.target,
            initial_packet.port,
            initial_packet.payload,
        ),
        flow_io,
    );

    Ok(PacketStream {
        sender,
        recv_tx: responses,
    })
}
