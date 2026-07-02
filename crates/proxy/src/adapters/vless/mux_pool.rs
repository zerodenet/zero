// VLESS MUX outbound connection pool bridge.
//
// Pool/cache state now lives in protocols/vless. This module only opens
// proxy-owned transport streams and hands them to the protocol pool.

use tokio::sync::mpsc;
use vless::mux_pool::{MuxIdentity, PoolKey, PoolKeyConfig};
use zero_config::{ClientTlsConfig, RealityConfig};
use zero_core::Session;
use zero_engine::EngineError;
use zero_platform_tokio::TransportConnector;

use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;

#[allow(clippy::too_many_arguments)]
pub(crate) async fn open_stream(
    pool: &vless::mux_pool::MuxConnectionPool,
    proxy: &Proxy,
    session: &Session,
    server: &str,
    port: u16,
    identity: MuxIdentity,
    tls: Option<&ClientTlsConfig>,
    reality: Option<&RealityConfig>,
    max_concurrency: u32,
) -> Result<TcpRelayStream, EngineError> {
    let key = pool_key(server, port, identity, tls, reality);
    let conn = pool
        .get_or_create_conn(key, max_concurrency, |key, max_concurrency| async move {
            create_mux_connection(proxy, &key, tls, reality, max_concurrency).await
        })
        .await?;

    let stream = vless::mux_pool::open_mux_tcp_stream(conn, session.port, &session.target)
        .map_err(|error| EngineError::Io(std::io::Error::other(error.to_string())))?;
    Ok(TcpRelayStream::new(stream))
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn open_udp_stream(
    pool: &vless::mux_pool::MuxConnectionPool,
    proxy: &Proxy,
    server: &str,
    port: u16,
    identity: MuxIdentity,
    tls: Option<&ClientTlsConfig>,
    reality: Option<&RealityConfig>,
    max_concurrency: u32,
) -> Result<
    (
        u16,
        mpsc::UnboundedSender<Vec<u8>>,
        mpsc::UnboundedReceiver<Vec<u8>>,
    ),
    EngineError,
> {
    let key = pool_key(server, port, identity, tls, reality);
    let conn = pool
        .get_or_create_conn(key, max_concurrency, |key, max_concurrency| async move {
            create_mux_connection(proxy, &key, tls, reality, max_concurrency).await
        })
        .await?;

    let stream = vless::mux_pool::open_mux_udp_stream(conn)
        .map_err(|error| EngineError::Io(std::io::Error::other(error.to_string())))?;
    Ok((stream.session_id, stream.up_tx, stream.down_rx))
}

fn pool_key(
    server: &str,
    port: u16,
    identity: MuxIdentity,
    tls: Option<&ClientTlsConfig>,
    reality: Option<&RealityConfig>,
) -> PoolKey {
    PoolKeyConfig::new(server, port, identity)
        .with_tls_server_name(tls.and_then(|config| config.server_name.as_deref()))
        .with_reality(
            reality.map(|config| config.public_key.as_str()),
            reality.and_then(|config| config.server_name.as_deref()),
        )
        .into_pool_key()
}

async fn create_mux_connection(
    proxy: &Proxy,
    key: &vless::mux_pool::PoolKey,
    tls: Option<&zero_config::ClientTlsConfig>,
    reality: Option<&zero_config::RealityConfig>,
    max_concurrency: u32,
) -> Result<vless::mux_pool::MuxPoolConn, EngineError> {
    use crate::transport::MeteredStream;

    let (server, port) = key.endpoint();
    let socket = proxy
        .protocols
        .direct_connector()
        .connect_host(server, port, proxy.resolver.as_ref())
        .await?;

    let connector =
        crate::transport::VlessTransportConnector::new(crate::transport::VlessTransportOptions {
            tls,
            reality,
            ws: None,
            grpc: None,
            h2: None,
            http_upgrade: None,
            split_http: None,
            source_dir: proxy.config.source_dir(),
        });
    let stream: TcpRelayStream = connector.connect(socket, server, port).await?;

    let mut metered = MeteredStream::new(stream);
    let _mux = key
        .establish_mux_connection(&mut metered)
        .await
        .map_err(|error| EngineError::Io(std::io::Error::other(error.to_string())))?;

    Ok(key
        .clone()
        .into_pool_conn(metered.into_inner(), max_concurrency))
}
