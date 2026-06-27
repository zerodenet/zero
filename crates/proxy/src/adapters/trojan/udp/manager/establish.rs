use super::connect;
use super::model::TrojanEntry;
use super::stream;
use crate::runtime::orchestration::OutboundEndpoint;
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;
use zero_core::Session;
use zero_engine::EngineError;

pub(super) async fn direct(
    proxy: &Proxy,
    session: &Session,
    endpoint: OutboundEndpoint<'_>,
    resume: &trojan::TrojanUdpFlowResume,
) -> Result<TrojanEntry, EngineError> {
    let tls_stream = connect::direct_tls_stream(proxy, endpoint, resume).await?;

    packet_stream(proxy, session, tls_stream, resume).await
}

pub(super) async fn over_relay_stream(
    stream: TcpRelayStream,
    tls_server_name: Option<&str>,
    proxy: &Proxy,
    session: &Session,
    endpoint: OutboundEndpoint<'_>,
    resume: &trojan::TrojanUdpFlowResume,
) -> Result<TrojanEntry, EngineError> {
    let tls_stream =
        connect::relay_tls_stream(stream, tls_server_name, proxy, endpoint, resume).await?;

    packet_stream(proxy, session, tls_stream, resume).await
}

async fn packet_stream(
    proxy: &Proxy,
    session: &Session,
    stream: TcpRelayStream,
    resume: &trojan::TrojanUdpFlowResume,
) -> Result<TrojanEntry, EngineError> {
    let stream::PacketStream { send_tx, recv_tx } =
        stream::spawn_packet_stream(proxy, session, stream, resume).await?;

    Ok(TrojanEntry { send_tx, recv_tx })
}
