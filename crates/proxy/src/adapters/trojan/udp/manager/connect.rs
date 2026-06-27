use crate::runtime::orchestration::OutboundEndpoint;
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;
use zero_engine::EngineError;

pub(super) async fn direct_tls_stream(
    proxy: &Proxy,
    endpoint: OutboundEndpoint<'_>,
    resume: &trojan::TrojanUdpFlowResume,
) -> Result<TcpRelayStream, EngineError> {
    crate::outbound::trojan::open_udp_tls_stream(proxy, endpoint, resume).await
}

pub(super) async fn relay_tls_stream(
    stream: TcpRelayStream,
    tls_server_name: Option<&str>,
    proxy: &Proxy,
    endpoint: OutboundEndpoint<'_>,
    resume: &trojan::TrojanUdpFlowResume,
) -> Result<TcpRelayStream, EngineError> {
    crate::outbound::trojan::open_udp_tls_relay_stream(
        stream,
        tls_server_name,
        proxy,
        endpoint,
        resume,
    )
    .await
}
