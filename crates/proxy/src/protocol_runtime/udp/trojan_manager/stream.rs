use super::bridge;
use super::socket::{ReadOnlySocket, WriteOnlySocket};
use crate::runtime::Proxy;
use crate::transport::{MeteredStream, TcpRelayStream};
use tokio::sync::{broadcast, mpsc};
use trojan::{TrojanOutbound, TrojanUdpPacket, TrojanUdpPacketTunnelTarget};
use zero_core::Session;
use zero_engine::EngineError;
use zero_traits::{UdpPacketStreamFraming, UdpPacketTunnelProtocol};

pub(super) struct PacketStream {
    pub(super) send_tx: mpsc::Sender<TrojanUdpPacket>,
    pub(super) recv_tx: broadcast::Sender<TrojanUdpPacket>,
}

pub(super) async fn spawn_packet_stream(
    proxy: &Proxy,
    session: &Session,
    stream: TcpRelayStream,
    password: &str,
) -> Result<PacketStream, EngineError> {
    let trojan = proxy.protocols.trojan_outbound_protocol();
    let mut metered = MeteredStream::new(stream);
    <TrojanOutbound as UdpPacketTunnelProtocol<TrojanUdpPacketTunnelTarget>>::establish_udp_packet_tunnel(
        &trojan,
        &mut metered,
        &TrojanUdpPacketTunnelTarget {
            session,
            password,
        },
    )
    .await?;

    let (read_half, write_half) = tokio::io::split(metered.into_inner());
    let (send_tx, send_rx) = mpsc::channel::<TrojanUdpPacket>(32);
    let recv_tx = bridge::response_channel();

    spawn_send_task(trojan, send_rx, WriteOnlySocket(write_half));
    spawn_recv_task(trojan, ReadOnlySocket(read_half), recv_tx.clone());

    Ok(PacketStream { send_tx, recv_tx })
}

fn spawn_send_task(
    trojan: TrojanOutbound,
    mut send_rx: mpsc::Receiver<TrojanUdpPacket>,
    mut send_stream: WriteOnlySocket,
) {
    tokio::spawn(async move {
        while let Some(packet) = send_rx.recv().await {
            if <TrojanOutbound as UdpPacketStreamFraming<TrojanUdpPacket>>::write_udp_packet(
                &trojan,
                &mut send_stream,
                &packet,
            )
            .await
            .is_err()
            {
                break;
            }
        }
    });
}

fn spawn_recv_task(
    trojan: TrojanOutbound,
    mut recv_stream: ReadOnlySocket,
    recv_tx: broadcast::Sender<TrojanUdpPacket>,
) {
    tokio::spawn(async move {
        while let Ok(packet) =
            <TrojanOutbound as UdpPacketStreamFraming<TrojanUdpPacket>>::read_udp_packet(
                &trojan,
                &mut recv_stream,
            )
            .await
        {
            if recv_tx.send(packet).is_err() {
                break;
            }
        }
    });
}
