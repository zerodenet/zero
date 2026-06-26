use super::super::MieruUdpPeer;
use super::connect;
use super::model::MieruEntry;
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
    let flow = mieru::open_udp_flow(stream, resume)
        .await
        .map_err(|error| {
            EngineError::Io(std::io::Error::other(format!(
                "mieru udp associate: {error}"
            )))
        })?;

    Ok(MieruEntry {
        sender: flow.sender,
        recv_tx: flow.responses,
    })
}
