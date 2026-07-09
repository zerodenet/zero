use std::{
    future::Future,
    path::{Path, PathBuf},
};

use zero_config::{ClientTlsConfig, GrpcConfig, WebSocketConfig};
use zero_engine::EngineError;
use zero_platform_tokio::{RelayCarrier, TcpRelayStream, TokioSocket};
use zero_traits::StreamMuxTransportHints;

use crate::outbound_stack::{
    connect_relay_transport_stack, connect_socket_transport_stack, StreamTransportStack,
};
use crate::transport_plan::TcpStreamTransportPlan;
#[derive(Clone, Copy)]
pub(super) struct VmessTransportOptions<'a> {
    tls: Option<&'a ClientTlsConfig>,
    ws: Option<&'a WebSocketConfig>,
    grpc: Option<&'a GrpcConfig>,
    source_dir: Option<&'a Path>,
}

#[derive(Debug, Clone)]
struct OwnedVmessTransportOptions {
    tls: Option<ClientTlsConfig>,
    ws: Option<WebSocketConfig>,
    grpc: Option<GrpcConfig>,
    source_dir: Option<PathBuf>,
}

impl OwnedVmessTransportOptions {
    fn from_borrowed(options: VmessTransportOptions<'_>) -> Self {
        Self {
            tls: options.tls.cloned(),
            ws: options.ws.cloned(),
            grpc: options.grpc.cloned(),
            source_dir: options.source_dir.map(PathBuf::from),
        }
    }

    fn as_borrowed(&self) -> VmessTransportOptions<'_> {
        VmessTransportOptions {
            tls: self.tls.as_ref(),
            ws: self.ws.as_ref(),
            grpc: self.grpc.as_ref(),
            source_dir: self.source_dir.as_deref(),
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct OwnedVmessOutboundTransportPlan {
    server: String,
    port: u16,
    transport: OwnedVmessTransportOptions,
}

impl OwnedVmessOutboundTransportPlan {
    pub(super) fn from_config_refs(
        source_dir: Option<&Path>,
        server: &str,
        port: u16,
        tls: Option<&ClientTlsConfig>,
        ws: Option<&WebSocketConfig>,
        grpc: Option<&GrpcConfig>,
    ) -> Self {
        Self::from_borrowed(
            server,
            port,
            VmessTransportOptions::from_config_refs(source_dir, tls, ws, grpc),
        )
    }

    fn from_borrowed(server: &str, port: u16, transport: VmessTransportOptions<'_>) -> Self {
        Self {
            server: server.to_owned(),
            port,
            transport: OwnedVmessTransportOptions::from_borrowed(transport),
        }
    }

    pub(super) fn server(&self) -> &str {
        &self.server
    }

    pub(super) fn port(&self) -> u16 {
        self.port
    }

    pub(super) fn transport(&self) -> VmessTransportOptions<'_> {
        self.transport.as_borrowed()
    }

    pub(super) fn mux_transport_hints(&self) -> StreamMuxTransportHints {
        let transport = self.transport();
        StreamMuxTransportHints::new(
            transport.tls.and_then(|config| config.server_name.clone()),
            transport.ws.map(|config| config.path.clone()),
            transport.grpc.map(|config| config.service_names.clone()),
            None,
            None,
        )
    }

    pub(super) async fn open_direct<OpenSocket, OpenSocketFut, E>(
        &self,
        open_socket: OpenSocket,
    ) -> Result<TcpRelayStream, EngineError>
    where
        OpenSocket: FnOnce(&str, u16) -> OpenSocketFut,
        OpenSocketFut: Future<Output = Result<TokioSocket, E>>,
        E: Into<EngineError>,
    {
        let socket = open_socket(self.server(), self.port())
            .await
            .map_err(Into::into)?;
        build_vmess_outbound_transport(VmessOutboundTransportRequest {
            socket,
            options: self.transport(),
            server: self.server(),
            port: self.port(),
        })
        .await
    }

    pub(super) async fn open_relay(
        &self,
        stream: TcpRelayStream,
    ) -> Result<TcpRelayStream, EngineError> {
        build_vmess_outbound_transport_over_stream(VmessFinalHopTransportRequest {
            carrier: RelayCarrier {
                stream,
                server: self.server().to_owned(),
                port: self.port(),
            },
            options: self.transport(),
        })
        .await
    }
}

impl TcpStreamTransportPlan for OwnedVmessOutboundTransportPlan {
    fn open_direct_stream<'a, OpenSocket, OpenSocketFut>(
        &'a self,
        open_socket: OpenSocket,
    ) -> crate::transport_plan::TransportOpenFuture<'a>
    where
        OpenSocket: FnOnce(&str, u16) -> OpenSocketFut + Send + 'a,
        OpenSocketFut: Future<Output = Result<TokioSocket, EngineError>> + Send + 'a,
    {
        Box::pin(async move { self.open_direct(open_socket).await })
    }

    fn open_relay_stream<'a>(
        &'a self,
        stream: TcpRelayStream,
    ) -> crate::transport_plan::TransportOpenFuture<'a> {
        Box::pin(async move { self.open_relay(stream).await })
    }
}

impl<'a> VmessTransportOptions<'a> {
    fn from_config_refs(
        source_dir: Option<&'a Path>,
        tls: Option<&'a ClientTlsConfig>,
        ws: Option<&'a WebSocketConfig>,
        grpc: Option<&'a GrpcConfig>,
    ) -> VmessTransportOptions<'a> {
        VmessTransportOptions {
            tls,
            ws,
            grpc,
            source_dir,
        }
    }
}

pub(super) struct VmessOutboundTransportRequest<'a> {
    pub(super) socket: TokioSocket,
    pub(super) options: VmessTransportOptions<'a>,
    pub(super) server: &'a str,
    pub(super) port: u16,
}

struct VmessFinalHopTransportRequest<'a> {
    carrier: RelayCarrier,
    options: VmessTransportOptions<'a>,
}

pub(super) async fn build_vmess_outbound_transport(
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
    connect_socket_transport_stack(
        socket,
        StreamTransportStack {
            tls: tls_config,
            ws: ws_config,
            grpc: grpc_config,
            h2: None,
            http_upgrade: None,
            source_dir,
        },
        server,
        port,
        "vmess: ws and grpc are mutually exclusive",
    )
    .await
}

async fn build_vmess_outbound_transport_over_stream(
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
    connect_relay_transport_stack(
        stream,
        StreamTransportStack {
            tls: tls_config,
            ws: ws_config,
            grpc: grpc_config,
            h2: None,
            http_upgrade: None,
            source_dir,
        },
        server,
        port,
        "vmess: ws and grpc are mutually exclusive",
    )
    .await
}
