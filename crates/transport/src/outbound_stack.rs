use std::io;
use std::path::Path;

use crate::RuntimeError;
use zero_platform_tokio::{TcpRelayStream, TokioSocket};
use zero_traits::{
    ClientTlsProfile, GrpcTransportProfile, H2TransportProfile, HttpUpgradeTransportProfile,
    WebSocketTransportProfile,
};

#[cfg(feature = "h2")]
use crate::h2;
#[cfg(feature = "http_upgrade")]
use crate::http_upgrade;
use crate::{grpc, tls, ws};

#[derive(Clone, Copy)]
pub struct StreamTransportStack<'a, TTls, TWs, TGrpc, TH2, THttp>
where
    TTls: ClientTlsProfile + ?Sized,
    TWs: WebSocketTransportProfile + ?Sized,
    TGrpc: GrpcTransportProfile + ?Sized,
    TH2: H2TransportProfile + ?Sized,
    THttp: HttpUpgradeTransportProfile + ?Sized,
{
    pub tls: Option<&'a TTls>,
    pub ws: Option<&'a TWs>,
    pub grpc: Option<&'a TGrpc>,
    pub h2: Option<&'a TH2>,
    pub http_upgrade: Option<&'a THttp>,
    pub source_dir: Option<&'a Path>,
}

pub async fn connect_socket_transport_stack<TTls, TWs, TGrpc, TH2, THttp>(
    socket: TokioSocket,
    stack: StreamTransportStack<'_, TTls, TWs, TGrpc, TH2, THttp>,
    server: &str,
    port: u16,
    invalid_message: &'static str,
) -> Result<TcpRelayStream, RuntimeError>
where
    TTls: ClientTlsProfile + ?Sized,
    TWs: WebSocketTransportProfile + ?Sized,
    TGrpc: GrpcTransportProfile + ?Sized,
    TH2: H2TransportProfile + ?Sized,
    THttp: HttpUpgradeTransportProfile + ?Sized,
{
    let StreamTransportStack {
        tls: tls_config,
        ws: ws_config,
        grpc: grpc_config,
        h2: h2_config,
        http_upgrade: http_upgrade_config,
        source_dir,
    } = stack;

    let carrier = match tls_config {
        Some(tls) => tls::connect_tls_upstream(socket, tls, source_dir, server).await?,
        None => TcpRelayStream::new(socket),
    };

    connect_layered_transport_stack(
        carrier,
        ws_config,
        grpc_config,
        h2_config,
        http_upgrade_config,
        server,
        port,
        invalid_message,
    )
    .await
}

pub async fn connect_relay_transport_stack<TTls, TWs, TGrpc, TH2, THttp>(
    stream: TcpRelayStream,
    stack: StreamTransportStack<'_, TTls, TWs, TGrpc, TH2, THttp>,
    server: &str,
    port: u16,
    invalid_message: &'static str,
) -> Result<TcpRelayStream, RuntimeError>
where
    TTls: ClientTlsProfile + ?Sized,
    TWs: WebSocketTransportProfile + ?Sized,
    TGrpc: GrpcTransportProfile + ?Sized,
    TH2: H2TransportProfile + ?Sized,
    THttp: HttpUpgradeTransportProfile + ?Sized,
{
    let StreamTransportStack {
        tls: tls_config,
        ws: ws_config,
        grpc: grpc_config,
        h2: h2_config,
        http_upgrade: http_upgrade_config,
        source_dir,
    } = stack;

    let carrier = match tls_config {
        Some(tls) => tls::connect_tls_stream(stream, tls, source_dir, server).await?,
        None => stream,
    };

    connect_layered_transport_stack(
        carrier,
        ws_config,
        grpc_config,
        h2_config,
        http_upgrade_config,
        server,
        port,
        invalid_message,
    )
    .await
}

async fn connect_layered_transport_stack<TWs, TGrpc, TH2, THttp>(
    carrier: TcpRelayStream,
    ws_config: Option<&TWs>,
    grpc_config: Option<&TGrpc>,
    h2_config: Option<&TH2>,
    http_upgrade_config: Option<&THttp>,
    server: &str,
    port: u16,
    invalid_message: &'static str,
) -> Result<TcpRelayStream, RuntimeError>
where
    TWs: WebSocketTransportProfile + ?Sized,
    TGrpc: GrpcTransportProfile + ?Sized,
    TH2: H2TransportProfile + ?Sized,
    THttp: HttpUpgradeTransportProfile + ?Sized,
{
    #[cfg(not(feature = "h2"))]
    if h2_config.is_some() {
        return invalid_transport_stack(invalid_message);
    }

    #[cfg(not(feature = "http_upgrade"))]
    if http_upgrade_config.is_some() {
        return invalid_transport_stack(invalid_message);
    }

    #[cfg(feature = "http_upgrade")]
    if let Some(config) = http_upgrade_config {
        if ws_config.is_some() || grpc_config.is_some() || h2_config.is_some() {
            return invalid_transport_stack(invalid_message);
        }
        return Ok(TcpRelayStream::new(
            http_upgrade::connect_http_upgrade(carrier, config).await?,
        ));
    }

    match (ws_config, grpc_config, h2_config) {
        (Some(ws), None, None) => Ok(TcpRelayStream::new(
            ws::connect_ws(carrier, ws, server, port).await?,
        )),
        (None, Some(grpc), None) => Ok(TcpRelayStream::new(
            grpc::connect_grpc(carrier, grpc.service_names()).await?,
        )),
        #[cfg(feature = "h2")]
        (None, None, Some(h2_config)) => Ok(TcpRelayStream::new(
            h2::connect_h2(carrier, h2_config, server, port).await?,
        )),
        (None, None, None) => Ok(carrier),
        _ => invalid_transport_stack(invalid_message),
    }
}

fn invalid_transport_stack(invalid_message: &'static str) -> Result<TcpRelayStream, RuntimeError> {
    Err(RuntimeError::Io(io::Error::new(
        io::ErrorKind::InvalidInput,
        invalid_message,
    )))
}
