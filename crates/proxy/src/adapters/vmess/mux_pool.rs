mod model;

use zero_core::Network;
use zero_engine::EngineError;
use zero_platform_tokio::TransportConnector;

use crate::transport::{MeteredStream, TcpRelayStream, VmessTransportConnector};

pub(crate) use model::VmessMuxOpenRequest;

pub(crate) async fn open_stream(
    pool: &vmess::mux::VmessMuxConnectionPool,
    request: VmessMuxOpenRequest<'_>,
) -> Result<TcpRelayStream, EngineError> {
    open_with_network(pool, request, Network::Tcp).await
}

pub(crate) async fn open_udp_stream(
    pool: &vmess::mux::VmessMuxConnectionPool,
    request: VmessMuxOpenRequest<'_>,
) -> Result<TcpRelayStream, EngineError> {
    open_with_network(pool, request, Network::Udp).await
}

async fn open_with_network(
    pool: &vmess::mux::VmessMuxConnectionPool,
    request: VmessMuxOpenRequest<'_>,
    network: Network,
) -> Result<TcpRelayStream, EngineError> {
    let key = request.pool_key().map_err(|error| {
        EngineError::Io(std::io::Error::new(std::io::ErrorKind::InvalidInput, error))
    })?;
    let target = request.session.target.clone();
    let port = request.session.port;
    let max_concurrency = request.max_concurrency;
    let conn = pool
        .get_or_create_conn(key, max_concurrency, |key, max_concurrency| async move {
            create_connection(
                request.proxy,
                &key,
                request.tls,
                request.ws,
                request.grpc,
                max_concurrency,
            )
            .await
        })
        .await?;

    Ok(TcpRelayStream::new(conn.open_stream(target, port, network)))
}

async fn create_connection(
    proxy: &crate::runtime::Proxy,
    key: &vmess::mux::VmessMuxPoolKey,
    tls: Option<&zero_config::ClientTlsConfig>,
    ws: Option<&zero_config::WebSocketConfig>,
    grpc: Option<&zero_config::GrpcConfig>,
    max_concurrency: u32,
) -> Result<vmess::mux::VmessMuxConn, EngineError> {
    let (server, port) = key.endpoint();
    let socket = proxy
        .protocols
        .direct_connector()
        .connect_host(server, port, proxy.resolver.as_ref())
        .await?;

    let connector = VmessTransportConnector::new(crate::transport::VmessTransportOptions {
        tls,
        ws,
        grpc,
        source_dir: proxy.config.source_dir(),
    });
    let stream = connector.connect(socket, server, port).await?;

    let metered = MeteredStream::new(stream);
    let stream = TcpRelayStream::new(key.establish_mux_outbound_stream(metered).await?);

    Ok(key.clone().into_pool_conn(stream, max_concurrency))
}
