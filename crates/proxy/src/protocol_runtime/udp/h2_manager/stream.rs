use super::bridge;
use crate::transport::Hysteria2Connector;
use std::sync::Arc;
use tokio::sync::mpsc;
use zero_engine::EngineError;

use super::super::packet_path_traits::UdpPacketRef;
use super::super::H2UdpPeer;

pub(super) struct PacketStream {
    pub(super) send_tx: mpsc::Sender<Vec<u8>>,
    pub(super) recv_tx: bridge::ResponseSender,
}

pub(super) async fn establish(
    peer: &H2UdpPeer<'_>,
    initial_packet: UdpPacketRef<'_>,
    resume: hysteria2::Hysteria2UdpFlowResume,
) -> Result<PacketStream, EngineError> {
    let connector =
        Hysteria2Connector::new(peer.endpoint.server, peer.endpoint.port, peer.password)
            .with_fingerprint(peer.client_fingerprint);
    let conn = Arc::new(connector.connect_raw().await?);

    let (send_tx, send_rx) = mpsc::channel::<Vec<u8>>(32);
    let recv_tx = bridge::response_channel();

    spawn_send_task(conn.clone(), send_rx, initial_packet, resume.clone());
    spawn_recv_task(conn, recv_tx.clone(), resume);

    Ok(PacketStream { send_tx, recv_tx })
}

fn spawn_send_task(
    conn: Arc<quinn::Connection>,
    mut send_rx: mpsc::Receiver<Vec<u8>>,
    initial_packet: UdpPacketRef<'_>,
    resume: hysteria2::Hysteria2UdpFlowResume,
) {
    let target_owned = initial_packet.target.clone();
    let port_owned = initial_packet.port;
    let init_payload = initial_packet.payload.to_vec();

    tokio::spawn(async move {
        if let Ok(datagram) = resume.encode_packet(&target_owned, port_owned, &init_payload) {
            if conn.send_datagram(datagram.into()).is_err() {
                return;
            }
        }
        while let Some(datagram) = send_rx.recv().await {
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
            if let Some((target, port, payload)) = resume.decode_packet(&data) {
                if recv_tx.send((target, port, payload)).is_err() {
                    break;
                }
            }
        }
    });
}
