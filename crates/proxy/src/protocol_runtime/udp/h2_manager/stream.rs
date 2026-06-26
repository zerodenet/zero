use super::super::H2UdpPeer;
use super::bridge;
use crate::outbound::hysteria2::Hysteria2Connector;
use crate::runtime::udp_flow::packet_path::UdpPacketRef;
use std::sync::Arc;
use tokio::sync::mpsc;
use zero_engine::EngineError;

pub(super) struct PacketStream {
    pub(super) send_tx: mpsc::Sender<hysteria2::Hysteria2UdpFlowPacket>,
    pub(super) recv_tx: bridge::ResponseSender,
}

pub(super) async fn establish(
    peer: &H2UdpPeer<'_>,
    initial_packet: UdpPacketRef<'_>,
    resume: hysteria2::Hysteria2UdpFlowResume,
) -> Result<PacketStream, EngineError> {
    let connector_profile = resume.connector_profile();
    let conn = Arc::new(
        Hysteria2Connector::new(
            peer.endpoint.server,
            peer.endpoint.port,
            connector_profile.password(),
        )
        .with_fingerprint(connector_profile.client_fingerprint())
        .connect_raw()
        .await?,
    );
    let initial_packet = hysteria2::udp_flow_packet(
        initial_packet.target,
        initial_packet.port,
        initial_packet.payload,
    );
    let (send_tx, send_rx) = mpsc::channel::<hysteria2::Hysteria2UdpFlowPacket>(32);
    let (recv_tx, _) = tokio::sync::broadcast::channel::<bridge::RecvItem>(32);

    spawn_send_task(conn.clone(), initial_packet, resume.clone(), send_rx);
    spawn_recv_task(conn, resume, recv_tx.clone());

    Ok(PacketStream { send_tx, recv_tx })
}

fn spawn_send_task(
    conn: Arc<quinn::Connection>,
    initial_packet: hysteria2::Hysteria2UdpFlowPacket,
    resume: hysteria2::Hysteria2UdpFlowResume,
    mut send_rx: mpsc::Receiver<hysteria2::Hysteria2UdpFlowPacket>,
) {
    tokio::spawn(async move {
        if let Ok(datagram) = initial_packet.encode_with(&resume) {
            if conn.send_datagram(datagram.into()).is_err() {
                return;
            }
        }
        while let Some(packet) = send_rx.recv().await {
            let Ok(datagram) = packet.encode_with(&resume) else {
                break;
            };
            if conn.send_datagram(datagram.into()).is_err() {
                break;
            }
        }
    });
}

fn spawn_recv_task(
    conn: Arc<quinn::Connection>,
    resume: hysteria2::Hysteria2UdpFlowResume,
    recv_tx: bridge::ResponseSender,
) {
    tokio::spawn(async move {
        while let Ok(data) = conn.read_datagram().await {
            let Some(packet) = resume.decode_flow_packet(&data) else {
                continue;
            };
            if recv_tx.send(packet.into_parts()).is_err() {
                break;
            }
        }
    });
}
