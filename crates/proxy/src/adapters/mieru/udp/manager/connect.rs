use crate::runtime::orchestration::OutboundEndpoint;
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;
use zero_engine::EngineError;

pub(super) async fn direct_stream(
    proxy: &Proxy,
    endpoint: OutboundEndpoint<'_>,
) -> Result<TcpRelayStream, EngineError> {
    let socket = proxy
        .protocols
        .direct_connector()
        .connect_host(endpoint.server, endpoint.port, proxy.resolver.as_ref())
        .await?;
    Ok(TcpRelayStream::new(socket))
}
