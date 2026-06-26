use super::super::MieruUdpPeer;
use super::connect;
use super::model::MieruEntry;
use super::stream;
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;
use zero_engine::EngineError;

pub(super) async fn direct(
    proxy: &Proxy,
    peer: &MieruUdpPeer<'_>,
) -> Result<MieruEntry, EngineError> {
    let stream = connect::direct_stream(proxy, peer).await?;
    packet_stream(stream, peer.resume).await
}

pub(super) async fn packet_stream(
    stream: TcpRelayStream,
    resume: &mieru::MieruUdpFlowResume,
) -> Result<MieruEntry, EngineError> {
    let connect::EstablishedSession { stream, flow_io } =
        connect::open_udp_flow(stream, resume).await?;
    let stream::PacketStream { send_tx, recv_tx } = stream::spawn_packet_stream(stream, flow_io);

    Ok(MieruEntry { send_tx, recv_tx })
}
