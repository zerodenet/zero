use std::{future::Future, io};

use crate::RuntimeError;
use zero_platform_tokio::{TcpRelayStream, TokioSocket};

pub async fn establish_mux_pool_connection<
    Transport,
    Stream,
    PoolConn,
    OpenSocket,
    OpenSocketFut,
    ConnectTransport,
    ConnectTransportFut,
    Finalize,
    FinalizeFut,
>(
    server: &str,
    port: u16,
    transport: Transport,
    max_concurrency: u32,
    open_socket: OpenSocket,
    connect_transport: ConnectTransport,
    finalize: Finalize,
) -> Result<PoolConn, RuntimeError>
where
    OpenSocket: FnOnce(&str, u16) -> OpenSocketFut,
    OpenSocketFut: Future<Output = Result<TokioSocket, RuntimeError>>,
    ConnectTransport: FnOnce(Transport, TokioSocket) -> ConnectTransportFut,
    ConnectTransportFut: Future<Output = Result<Stream, RuntimeError>>,
    Finalize: FnOnce(Stream, u32) -> FinalizeFut,
    FinalizeFut: Future<Output = Result<PoolConn, RuntimeError>>,
{
    let socket = open_socket(server, port).await?;
    let stream = connect_transport(transport, socket).await?;
    finalize(stream, max_concurrency).await
}

pub async fn use_mux_pool_connection<
    Key,
    Connection,
    Output,
    LoadConnection,
    LoadConnectionFut,
    UseConnection,
>(
    key: Key,
    max_concurrency: u32,
    load_connection: LoadConnection,
    use_connection: UseConnection,
) -> Result<Output, RuntimeError>
where
    LoadConnection: FnOnce(Key, u32) -> LoadConnectionFut,
    LoadConnectionFut: Future<Output = Result<Connection, RuntimeError>>,
    UseConnection: FnOnce(Connection) -> Result<Output, RuntimeError>,
{
    let conn = load_connection(key, max_concurrency).await?;
    use_connection(conn)
}

pub async fn open_cached_mux_pool_connection<
    Key,
    CachedConnection,
    Output,
    CreateConnection,
    CreateConnectionFut,
    GetOrCreateCachedConnection,
    GetOrCreateCachedConnectionFut,
    UseConnection,
>(
    key: Key,
    max_concurrency: u32,
    get_or_create_cached_connection: GetOrCreateCachedConnection,
    create_connection: CreateConnection,
    use_connection: UseConnection,
) -> Result<Output, RuntimeError>
where
    GetOrCreateCachedConnection:
        FnOnce(Key, u32, CreateConnection) -> GetOrCreateCachedConnectionFut,
    GetOrCreateCachedConnectionFut: Future<Output = Result<CachedConnection, RuntimeError>>,
    CreateConnection: FnOnce(Key, u32) -> CreateConnectionFut,
    CreateConnectionFut: Future,
    UseConnection: FnOnce(CachedConnection) -> Result<Output, RuntimeError>,
{
    use_mux_pool_connection(
        key,
        max_concurrency,
        |key, max_concurrency| {
            get_or_create_cached_connection(key, max_concurrency, create_connection)
        },
        use_connection,
    )
    .await
}

pub fn map_transport_connector_error(error: RuntimeError) -> io::Error {
    match error {
        RuntimeError::Io(io_error) => io_error,
        other => io::Error::other(other),
    }
}

pub async fn connect_transport_connector<BuildTransport, BuildTransportFut>(
    socket: TokioSocket,
    build_transport: BuildTransport,
) -> io::Result<TcpRelayStream>
where
    BuildTransport: FnOnce(TokioSocket) -> BuildTransportFut,
    BuildTransportFut: Future<Output = Result<TcpRelayStream, RuntimeError>>,
{
    build_transport(socket)
        .await
        .map_err(map_transport_connector_error)
}
