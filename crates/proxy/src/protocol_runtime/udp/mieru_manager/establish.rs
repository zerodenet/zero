use super::connect;
use super::model::{MieruEntry, MieruUdpPeer};
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
    let flow = stream::spawn_packet_stream(stream, resume).await?;

    Ok(MieruEntry {
        sender: flow.sender,
        recv_tx: flow.recv_tx,
    })
}
