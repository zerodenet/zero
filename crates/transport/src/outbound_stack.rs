use std::io;
use std::path::Path;

use zero_config::{ClientTlsConfig, GrpcConfig, H2Config, HttpUpgradeConfig, WebSocketConfig};
use zero_engine::EngineError;
use zero_platform_tokio::{TcpRelayStream, TokioSocket};

#[cfg(feature = "h2")]
use crate::h2;
#[cfg(feature = "http_upgrade")]
use crate::http_upgrade;
use crate::{grpc, tls, ws};

#[derive(Clone, Copy)]
pub struct StreamTransportStack<'a> {
    pub tls: Option<&'a ClientTlsConfig>,
    pub ws: Option<&'a WebSocketConfig>,
    pub grpc: Option<&'a GrpcConfig>,
    pub h2: Option<&'a H2Config>,
    pub http_upgrade: Option<&'a HttpUpgradeConfig>,
    pub source_dir: Option<&'a Path>,
}

pub async fn connect_socket_transport_stack(
    socket: TokioSocket,
    stack: StreamTransportStack<'_>,
    server: &str,
    port: u16,
    invalid_message: &'static str,
) -> Result<TcpRelayStream, EngineError> {
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

pub async fn connect_relay_transport_stack(
    stream: TcpRelayStream,
    stack: StreamTransportStack<'_>,
    server: &str,
    port: u16,
    invalid_message: &'static str,
) -> Result<TcpRelayStream, EngineError> {
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

async fn connect_layered_transport_stack(
    carrier: TcpRelayStream,
    ws_config: Option<&WebSocketConfig>,
    grpc_config: Option<&GrpcConfig>,
    h2_config: Option<&H2Config>,
    http_upgrade_config: Option<&HttpUpgradeConfig>,
    server: &str,
    port: u16,
    invalid_message: &'static str,
) -> Result<TcpRelayStream, EngineError> {
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
            grpc::connect_grpc(carrier, &grpc.service_names).await?,
        )),
        #[cfg(feature = "h2")]
        (None, None, Some(h2_config)) => Ok(TcpRelayStream::new(
            h2::connect_h2(carrier, h2_config, server, port).await?,
        )),
        (None, None, None) => Ok(carrier),
        _ => invalid_transport_stack(invalid_message),
    }
}

fn invalid_transport_stack(invalid_message: &'static str) -> Result<TcpRelayStream, EngineError> {
    Err(EngineError::Io(io::Error::new(
        io::ErrorKind::InvalidInput,
        invalid_message,
    )))
}
