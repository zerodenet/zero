use super::bridge;
use crate::transport::Hysteria2Connector;
use std::sync::Arc;
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
    let connector_profile = peer.resume.connector_profile();
    let connector = Hysteria2Connector::new(
        peer.endpoint.server,
        peer.endpoint.port,
        connector_profile.password(),
    )
    .with_fingerprint(connector_profile.client_fingerprint());
    let conn = Arc::new(connector.connect_raw().await?);

    let (send_tx, send_rx) = mpsc::channel::<hysteria2::Hysteria2UdpFlowPacket>(32);
    let recv_tx = bridge::response_channel();

    spawn_send_task(conn.clone(), send_rx, initial_packet, resume.clone());
    spawn_recv_task(conn, recv_tx.clone(), resume);

    Ok(PacketStream { send_tx, recv_tx })
}

fn spawn_send_task(
    conn: Arc<quinn::Connection>,
    mut send_rx: mpsc::Receiver<hysteria2::Hysteria2UdpFlowPacket>,
    initial_packet: UdpPacketRef<'_>,
    resume: hysteria2::Hysteria2UdpFlowResume,
) {
    let initial_packet = hysteria2::Hysteria2UdpFlowPacket::from_parts(
        initial_packet.target,
        initial_packet.port,
        initial_packet.payload,
    );

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
    recv_tx: bridge::ResponseSender,
    resume: hysteria2::Hysteria2UdpFlowResume,
) {
    tokio::spawn(async move {
        while let Ok(data) = conn.read_datagram().await {
            if let Some(packet) = resume.decode_flow_packet(&data) {
                let (target, port, payload) = packet.into_parts();
                if recv_tx.send((target, port, payload)).is_err() {
                    break;
                }
            }
        }
    });
}
