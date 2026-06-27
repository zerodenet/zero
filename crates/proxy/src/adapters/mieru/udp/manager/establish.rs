use super::connect;
use super::model::MieruEntry;
use super::stream;
use crate::runtime::orchestration::OutboundEndpoint;
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;
use zero_engine::EngineError;

pub(super) async fn direct(
    proxy: &Proxy,
    endpoint: OutboundEndpoint<'_>,
    resume: &mieru::MieruUdpFlowResume,
) -> Result<MieruEntry, EngineError> {
    let stream = connect::direct_stream(proxy, endpoint).await?;
    packet_stream(stream, resume).await
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
