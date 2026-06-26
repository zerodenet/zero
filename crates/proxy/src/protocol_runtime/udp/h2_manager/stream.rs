use super::bridge;
use crate::outbound::hysteria2::Hysteria2Connector;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use zero_core::{Error, UdpFlowPacket};
use zero_engine::EngineError;

use super::super::packet_path_traits::UdpPacketRef;
use super::super::H2UdpPeer;

pub(super) struct PacketStream {
    pub(super) send_tx: mpsc::Sender<UdpFlowPacket>,
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
    let initial_packet = UdpFlowPacket::from_parts(
        initial_packet.target,
        initial_packet.port,
        initial_packet.payload,
    );
    let (send_tx, send_rx) = mpsc::channel::<UdpFlowPacket>(32);
    let (recv_tx, _) = broadcast::channel(32);

    spawn_send_task(conn.clone(), send_rx, initial_packet, resume.clone());
    spawn_recv_task(conn, recv_tx.clone(), resume);

    Ok(PacketStream { send_tx, recv_tx })
}

fn spawn_send_task(
    conn: Arc<quinn::Connection>,
    mut send_rx: mpsc::Receiver<UdpFlowPacket>,
    initial_packet: UdpFlowPacket,
    resume: hysteria2::Hysteria2UdpFlowResume,
) {
    tokio::spawn(async move {
        if let Ok(datagram) = encode_packet(initial_packet, &resume) {
            if conn.send_datagram(datagram.into()).is_err() {
                return;
            }
        }
        while let Some(packet) = send_rx.recv().await {
            let Ok(datagram) = encode_packet(packet, &resume) else {
                break;
            };
            if conn.send_datagram(datagram.into()).is_err() {
                break;
            }
        }
    });
}

fn encode_packet(
    packet: UdpFlowPacket,
    resume: &hysteria2::Hysteria2UdpFlowResume,
) -> Result<Vec<u8>, Error> {
    let packet = hysteria2::udp_flow_packet(&packet.target, packet.port, &packet.payload);
    packet.encode_with(resume)
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
