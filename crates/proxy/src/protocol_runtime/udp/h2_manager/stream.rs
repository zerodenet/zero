use super::bridge;
use crate::transport::{establish_hysteria2_udp_flow_stream, Hysteria2UdpFlowStreamRequest};
use tokio::sync::mpsc;
use zero_engine::EngineError;

use super::super::packet_path_traits::UdpPacketRef;
use super::super::H2UdpPeer;

pub(super) struct PacketStream {
    pub(super) send_tx: mpsc::Sender<hysteria2::Hysteria2UdpFlowPacket>,
    pub(super) recv_tx: bridge::ResponseSender,
}

pub(super) async fn establish(
    peer: &H2UdpPeer<'_>,
    initial_packet: UdpPacketRef<'_>,
    resume: hysteria2::Hysteria2UdpFlowResume,
) -> Result<PacketStream, EngineError> {
    let initial_packet = hysteria2::udp_flow_packet(
        initial_packet.target,
        initial_packet.port,
        initial_packet.payload,
    );
    let stream = establish_hysteria2_udp_flow_stream(Hysteria2UdpFlowStreamRequest {
        server: peer.endpoint.server.to_owned(),
        port: peer.endpoint.port,
        resume,
        initial_packet,
    })
    .await?;

    Ok(PacketStream {
        send_tx: stream.send_tx,
        recv_tx: stream.recv_tx,
    })
}
