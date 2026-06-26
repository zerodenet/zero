//! Unified VMess outbound transport builder.

use std::{io, path::Path};

use zero_config::{ClientTlsConfig, GrpcConfig, WebSocketConfig};
use zero_engine::EngineError;
use zero_platform_tokio::{RelayCarrier, TcpRelayStream, TokioSocket, TransportConnector};

use crate::{grpc, tls, ws};

#[derive(Clone, Copy)]
pub struct VmessTransportOptions<'a> {
    pub tls: Option<&'a ClientTlsConfig>,
    pub ws: Option<&'a WebSocketConfig>,
    pub grpc: Option<&'a GrpcConfig>,
    pub source_dir: Option<&'a Path>,
}

pub struct VmessOutboundTransportRequest<'a> {
    pub socket: TokioSocket,
    pub options: VmessTransportOptions<'a>,
    pub server: &'a str,
    pub port: u16,
}

pub struct VmessFinalHopTransportRequest<'a> {
    pub carrier: RelayCarrier,
    pub options: VmessTransportOptions<'a>,
}

pub async fn build_vmess_outbound_transport(
    request: VmessOutboundTransportRequest<'_>,
) -> Result<TcpRelayStream, EngineError> {
    let VmessOutboundTransportRequest {
        socket,
        options,
        server,
        port,
    } = request;
    let VmessTransportOptions {
        tls: tls_config,
        ws: ws_config,
        grpc: grpc_config,
        source_dir,
    } = options;

    match (tls_config, ws_config, grpc_config) {
        (Some(tls), None, Some(grpc)) => {
            let tls_stream = tls::connect_tls_upstream(socket, tls, source_dir, server).await?;
            Ok(TcpRelayStream::new(
                grpc::connect_grpc(tls_stream, &grpc.service_names).await?,
            ))
        }
        (None, None, Some(grpc)) => Ok(TcpRelayStream::new(
            grpc::connect_grpc(socket, &grpc.service_names).await?,
        )),
        (Some(tls), Some(ws), None) => {
            let tls_stream = tls::connect_tls_upstream(socket, tls, source_dir, server).await?;
            Ok(TcpRelayStream::new(
                ws::connect_ws(tls_stream, ws, server, port).await?,
            ))
        }
        (None, Some(ws), None) => Ok(TcpRelayStream::new(
            ws::connect_ws(socket, ws, server, port).await?,
        )),
        (Some(tls), None, None) => tls::connect_tls_upstream(socket, tls, source_dir, server).await,
        (None, None, None) => Ok(socket.into()),
        _ => Err(EngineError::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            "vmess: ws and grpc are mutually exclusive",
        ))),
    }
}

pub async fn build_vmess_outbound_transport_over_stream(
    request: VmessFinalHopTransportRequest<'_>,
) -> Result<TcpRelayStream, EngineError> {
    let VmessFinalHopTransportRequest { carrier, options } = request;
    let RelayCarrier {
        stream,
        server,
        port,
    } = carrier;
    let server: &str = &server;
    let VmessTransportOptions {
        tls: tls_config,
        ws: ws_config,
        grpc: grpc_config,
        source_dir,
    } = options;

    match (tls_config, ws_config, grpc_config) {
        (Some(tls), None, Some(grpc)) => {
            let tls_stream = tls::connect_tls_stream(stream, tls, source_dir, server).await?;
            Ok(TcpRelayStream::new(
                grpc::connect_grpc(tls_stream, &grpc.service_names).await?,
            ))
        }
        (None, None, Some(grpc)) => Ok(TcpRelayStream::new(
            grpc::connect_grpc(stream, &grpc.service_names).await?,
        )),
        (Some(tls), Some(ws), None) => {
            let tls_stream = tls::connect_tls_stream(stream, tls, source_dir, server).await?;
            Ok(TcpRelayStream::new(
                ws::connect_ws(tls_stream, ws, server, port).await?,
            ))
        }
        (None, Some(ws), None) => Ok(TcpRelayStream::new(
            ws::connect_ws(stream, ws, server, port).await?,
        )),
        (Some(tls), None, None) => tls::connect_tls_stream(stream, tls, source_dir, server).await,
        (None, None, None) => Ok(TcpRelayStream::new(stream)),
        _ => Err(EngineError::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            "vmess: ws and grpc are mutually exclusive",
        ))),
    }
}

pub struct VmessTransportConnector<'a> {
    options: VmessTransportOptions<'a>,
}

impl<'a> VmessTransportConnector<'a> {
    pub fn new(options: VmessTransportOptions<'a>) -> Self {
        Self { options }
    }
}

impl TransportConnector for VmessTransportConnector<'_> {
    type Stream = TcpRelayStream;

    async fn connect(
        &self,
        socket: TokioSocket,
        server: &str,
        port: u16,
    ) -> io::Result<Self::Stream> {
        build_vmess_outbound_transport(VmessOutboundTransportRequest {
            socket,
            options: self.options,
            server,
            port,
        })
        .await
        .map_err(|error| match error {
            EngineError::Io(io_error) => io_error,
            other => io::Error::other(other),
        })
    }
}
