use super::bridge;
use super::model::H2UdpPeer;
use crate::outbound::hysteria2::Hysteria2Connector;
use crate::runtime::udp_flow::packet_path::UdpPacketRef;
use std::sync::Arc;
use tokio::sync::mpsc;
use zero_core::UdpFlowPacket;
use zero_engine::EngineError;

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
    let flow_io = resume.flow_io();
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
    let (send_tx, send_rx) = mpsc::channel::<UdpFlowPacket>(32);
    let (recv_tx, _) = tokio::sync::broadcast::channel::<bridge::RecvItem>(32);

    spawn_send_task(
        conn.clone(),
        UdpFlowPacket::from_parts(
            initial_packet.target,
            initial_packet.port,
            initial_packet.payload,
        ),
        flow_io,
        send_rx,
    );
    spawn_recv_task(conn, flow_io, recv_tx.clone());

    Ok(PacketStream { send_tx, recv_tx })
}

fn spawn_send_task(
    conn: Arc<quinn::Connection>,
    initial_packet: UdpFlowPacket,
    flow_io: hysteria2::Hysteria2UdpFlowIo,
    mut send_rx: mpsc::Receiver<UdpFlowPacket>,
) {
    tokio::spawn(async move {
        if let Ok(datagram) = flow_io.encode_packet(&initial_packet) {
            if conn.send_datagram(datagram.into()).is_err() {
                return;
            }
        }
        while let Some(packet) = send_rx.recv().await {
            let Ok(datagram) = flow_io.encode_packet(&packet) else {
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
    flow_io: hysteria2::Hysteria2UdpFlowIo,
    recv_tx: bridge::ResponseSender,
) {
    tokio::spawn(async move {
        while let Ok(data) = conn.read_datagram().await {
            let Some(packet) = flow_io.decode_packet(&data) else {
                continue;
            };
            if recv_tx
                .send((packet.target, packet.port, packet.payload))
                .is_err()
            {
                break;
            }
        }
    });
}
