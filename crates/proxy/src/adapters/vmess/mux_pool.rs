use zero_config::{ClientTlsConfig, GrpcConfig, WebSocketConfig};
use zero_core::{Error, Network, Session};
use zero_engine::EngineError;
use zero_platform_tokio::TransportConnector;

use crate::transport::{MeteredStream, TcpRelayStream, VmessTransportConnector};

#[allow(clippy::too_many_arguments)]
pub(crate) async fn open_stream(
    pool: &vmess::mux::VmessMuxConnectionPool,
    proxy: &crate::runtime::Proxy,
    session: &Session,
    server: &str,
    port: u16,
    identity: vmess::mux::VmessMuxIdentity,
    tls: Option<&ClientTlsConfig>,
    ws: Option<&WebSocketConfig>,
    grpc: Option<&GrpcConfig>,
    max_concurrency: u32,
) -> Result<TcpRelayStream, EngineError> {
    open_with_network(
        pool,
        proxy,
        session,
        server,
        port,
        identity,
        tls,
        ws,
        grpc,
        max_concurrency,
        Network::Tcp,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn open_udp_stream(
    pool: &vmess::mux::VmessMuxConnectionPool,
    proxy: &crate::runtime::Proxy,
    session: &Session,
    server: &str,
    port: u16,
    identity: vmess::mux::VmessMuxIdentity,
    tls: Option<&ClientTlsConfig>,
    ws: Option<&WebSocketConfig>,
    grpc: Option<&GrpcConfig>,
    max_concurrency: u32,
) -> Result<TcpRelayStream, EngineError> {
    open_with_network(
        pool,
        proxy,
        session,
        server,
        port,
        identity,
        tls,
        ws,
        grpc,
        max_concurrency,
        Network::Udp,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn open_with_network(
    pool: &vmess::mux::VmessMuxConnectionPool,
    proxy: &crate::runtime::Proxy,
    session: &Session,
    server: &str,
    port: u16,
    identity: vmess::mux::VmessMuxIdentity,
    tls: Option<&ClientTlsConfig>,
    ws: Option<&WebSocketConfig>,
    grpc: Option<&GrpcConfig>,
    max_concurrency: u32,
    network: Network,
) -> Result<TcpRelayStream, EngineError> {
    let key = pool_key(server, port, identity, tls, ws, grpc).map_err(|error| {
        EngineError::Io(std::io::Error::new(std::io::ErrorKind::InvalidInput, error))
    })?;
    let target = session.target.clone();
    let target_port = session.port;
    let conn = pool
        .get_or_create_conn(key, max_concurrency, |key, max_concurrency| async move {
            create_connection(proxy, &key, tls, ws, grpc, max_concurrency).await
        })
        .await?;

    Ok(TcpRelayStream::new(conn.open_stream(
        target,
        target_port,
        network,
    )))
}

fn pool_key(
    server: &str,
    port: u16,
    identity: vmess::mux::VmessMuxIdentity,
    tls: Option<&ClientTlsConfig>,
    ws: Option<&WebSocketConfig>,
    grpc: Option<&GrpcConfig>,
) -> Result<vmess::mux::VmessMuxPoolKey, Error> {
    vmess::mux::VmessMuxPoolKeyConfig::new(server.to_owned(), port, identity)
        .with_tls_server_name(tls.and_then(|config| config.server_name.as_deref()))
        .with_ws_path(ws.map(|config| config.path.as_str()))
        .with_grpc_service_names(grpc.map(|config| config.service_names.clone()))
        .into_pool_key()
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
