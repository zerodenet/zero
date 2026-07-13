use std::future::Future;
use std::path::{Path, PathBuf};

use zero_config::{
    ClientTlsConfig, GrpcConfig, H2Config, HttpUpgradeConfig, QuicConfig, RealityConfig,
    SplitHttpConfig, WebSocketConfig,
};
use zero_engine::EngineError;
use zero_platform_tokio::{RelayCarrier, TcpRelayStream, TokioSocket};
use zero_traits::StreamMuxTransportHints;

use crate::split_http;
use crate::transport_plan::TcpStreamTransportPlan;

use super::{
    build_vless_direct_outbound_transport, build_vless_outbound_transport_over_stream,
    build_vless_split_http_over_relay, build_vless_udp_outbound_transport,
};

#[derive(Clone, Copy)]
pub(in crate::vless_transport) struct VlessTransportOptions<'a> {
    pub(super) tls: Option<&'a ClientTlsConfig>,
    pub(super) reality: Option<&'a RealityConfig>,
    pub(super) ws: Option<&'a WebSocketConfig>,
    pub(super) grpc: Option<&'a GrpcConfig>,
    pub(super) h2: Option<&'a H2Config>,
    pub(super) http_upgrade: Option<&'a HttpUpgradeConfig>,
    pub(super) split_http: Option<&'a SplitHttpConfig>,
    pub(super) source_dir: Option<&'a Path>,
}

impl<'a> VlessTransportOptions<'a> {
    pub(in crate::vless_transport) fn uses_deferred_tcp_response(self) -> bool {
        self.reality.is_some()
    }
}

pub(in crate::vless_transport) struct VlessOutboundTransportRequest<'a> {
    pub(super) socket: TokioSocket,
    pub(super) options: VlessTransportOptions<'a>,
    pub(super) server: &'a str,
    pub(super) port: u16,
}

pub(in crate::vless_transport) struct VlessDirectTransportRequest<'a> {
    pub(super) socket: Option<TokioSocket>,
    pub(super) options: VlessTransportOptions<'a>,
    pub(super) quic: Option<&'a QuicConfig>,
    pub(super) server: &'a str,
    pub(super) port: u16,
}

pub(in crate::vless_transport) struct VlessFinalHopTransportRequest<'a> {
    pub(super) carrier: RelayCarrier,
    pub(super) options: VlessTransportOptions<'a>,
}

#[derive(Clone, Copy)]
pub(in crate::vless_transport) struct VlessUdpTransportOptions<'a> {
    pub(super) tls: Option<&'a ClientTlsConfig>,
    pub(super) reality: Option<&'a RealityConfig>,
    pub(super) ws: Option<&'a WebSocketConfig>,
    pub(super) grpc: Option<&'a GrpcConfig>,
    pub(super) h2: Option<&'a H2Config>,
    pub(super) http_upgrade: Option<&'a HttpUpgradeConfig>,
    pub(super) split_http: Option<&'a SplitHttpConfig>,
    pub(super) quic: Option<&'a QuicConfig>,
    pub(super) source_dir: Option<&'a Path>,
}

impl<'a> VlessUdpTransportOptions<'a> {
    #[allow(clippy::too_many_arguments)]
    fn from_config_refs(
        source_dir: Option<&'a Path>,
        tls: Option<&'a ClientTlsConfig>,
        reality: Option<&'a RealityConfig>,
        ws: Option<&'a WebSocketConfig>,
        grpc: Option<&'a GrpcConfig>,
        h2: Option<&'a H2Config>,
        http_upgrade: Option<&'a HttpUpgradeConfig>,
        split_http: Option<&'a SplitHttpConfig>,
        quic: Option<&'a QuicConfig>,
    ) -> Self {
        Self {
            tls,
            reality,
            ws,
            grpc,
            h2,
            http_upgrade,
            split_http,
            quic,
            source_dir,
        }
    }

    pub(in crate::vless_transport) fn stream_options(self) -> VlessTransportOptions<'a> {
        VlessTransportOptions {
            tls: self.tls,
            reality: self.reality,
            ws: self.ws,
            grpc: self.grpc,
            h2: self.h2,
            http_upgrade: self.http_upgrade,
            split_http: self.split_http,
            source_dir: self.source_dir,
        }
    }

    fn uses_paired_relay_transport(self) -> bool {
        self.split_http
            .is_some_and(|cfg| !split_http::XhttpMode::parse(&cfg.mode).is_single_connection())
    }
}

#[derive(Debug, Clone)]
struct OwnedVlessUdpTransportOptions {
    tls: Option<ClientTlsConfig>,
    reality: Option<RealityConfig>,
    ws: Option<WebSocketConfig>,
    grpc: Option<GrpcConfig>,
    h2: Option<H2Config>,
    http_upgrade: Option<HttpUpgradeConfig>,
    split_http: Option<SplitHttpConfig>,
    quic: Option<QuicConfig>,
    source_dir: Option<PathBuf>,
}

impl OwnedVlessUdpTransportOptions {
    fn from_borrowed(options: VlessUdpTransportOptions<'_>) -> Self {
        Self {
            tls: options.tls.cloned(),
            reality: options.reality.cloned(),
            ws: options.ws.cloned(),
            grpc: options.grpc.cloned(),
            h2: options.h2.cloned(),
            http_upgrade: options.http_upgrade.cloned(),
            split_http: options.split_http.cloned(),
            quic: options.quic.cloned(),
            source_dir: options.source_dir.map(PathBuf::from),
        }
    }

    fn as_borrowed(&self) -> VlessUdpTransportOptions<'_> {
        VlessUdpTransportOptions {
            tls: self.tls.as_ref(),
            reality: self.reality.as_ref(),
            ws: self.ws.as_ref(),
            grpc: self.grpc.as_ref(),
            h2: self.h2.as_ref(),
            http_upgrade: self.http_upgrade.as_ref(),
            split_http: self.split_http.as_ref(),
            quic: self.quic.as_ref(),
            source_dir: self.source_dir.as_deref(),
        }
    }

    fn stream_options(&self) -> VlessTransportOptions<'_> {
        self.as_borrowed().stream_options()
    }
}

#[derive(Debug, Clone)]
pub struct OwnedVlessOutboundTransportPlan {
    server: String,
    pub(super) port: u16,
    transport: OwnedVlessUdpTransportOptions,
}

impl OwnedVlessOutboundTransportPlan {
    #[allow(clippy::too_many_arguments)]
    pub fn from_config_refs(
        source_dir: Option<&Path>,
        server: &str,
        port: u16,
        tls: Option<&ClientTlsConfig>,
        reality: Option<&RealityConfig>,
        ws: Option<&WebSocketConfig>,
        grpc: Option<&GrpcConfig>,
        h2: Option<&H2Config>,
        http_upgrade: Option<&HttpUpgradeConfig>,
        split_http: Option<&SplitHttpConfig>,
        quic: Option<&QuicConfig>,
    ) -> Self {
        Self::from_borrowed(
            server,
            port,
            VlessUdpTransportOptions::from_config_refs(
                source_dir,
                tls,
                reality,
                ws,
                grpc,
                h2,
                http_upgrade,
                split_http,
                quic,
            ),
        )
    }

    fn from_borrowed(server: &str, port: u16, transport: VlessUdpTransportOptions<'_>) -> Self {
        Self {
            server: server.to_owned(),
            port,
            transport: OwnedVlessUdpTransportOptions::from_borrowed(transport),
        }
    }

    pub(in crate::vless_transport) fn server(&self) -> &str {
        &self.server
    }

    pub(in crate::vless_transport) fn port(&self) -> u16 {
        self.port
    }

    pub(in crate::vless_transport) fn transport(&self) -> VlessUdpTransportOptions<'_> {
        self.transport.as_borrowed()
    }

    pub(in crate::vless_transport) fn stream_transport_options(&self) -> VlessTransportOptions<'_> {
        self.transport.stream_options()
    }

    pub(in crate::vless_transport) fn uses_deferred_tcp_response(&self) -> bool {
        self.stream_transport_options().uses_deferred_tcp_response()
    }

    pub(in crate::vless_transport) fn uses_quic(&self) -> bool {
        self.transport().quic.is_some()
    }

    pub(in crate::vless_transport) fn relay_needs_two_streams(&self) -> bool {
        self.transport().uses_paired_relay_transport()
    }

    pub fn mux_transport_hints(&self) -> StreamMuxTransportHints {
        let transport = self.stream_transport_options();
        StreamMuxTransportHints::new(
            transport.tls.and_then(|config| config.server_name.clone()),
            None,
            None,
            transport.reality.map(|config| config.public_key.clone()),
            transport
                .reality
                .and_then(|config| config.server_name.clone()),
        )
    }

    pub(in crate::vless_transport) async fn open_direct<OpenSocket, OpenSocketFut, E>(
        &self,
        open_socket: OpenSocket,
    ) -> Result<TcpRelayStream, EngineError>
    where
        OpenSocket: FnOnce(&str, u16) -> OpenSocketFut,
        OpenSocketFut: Future<Output = Result<TokioSocket, E>>,
        E: Into<EngineError>,
    {
        let transport = self.transport();
        if transport.quic.is_some() {
            let quic = transport.quic;
            return build_vless_direct_outbound_transport(VlessDirectTransportRequest {
                socket: None,
                options: transport.stream_options(),
                quic,
                server: self.server(),
                port: self.port(),
            })
            .await;
        }

        let socket = open_socket(self.server(), self.port())
            .await
            .map_err(Into::into)?;
        build_vless_udp_outbound_transport(VlessUdpOutboundTransportRequest {
            socket,
            options: transport,
            server: self.server(),
            port: self.port(),
        })
        .await
    }

    pub(in crate::vless_transport) async fn open_relay(
        &self,
        stream: TcpRelayStream,
    ) -> Result<TcpRelayStream, EngineError> {
        build_vless_outbound_transport_over_stream(VlessFinalHopTransportRequest {
            carrier: RelayCarrier {
                stream,
                server: self.server().to_owned(),
                port: self.port(),
            },
            options: self.stream_transport_options(),
        })
        .await
    }

    pub(in crate::vless_transport) async fn build_relay_two_stream_udp_transport(
        &self,
        post_stream: TcpRelayStream,
        get_stream: TcpRelayStream,
    ) -> Result<TcpRelayStream, EngineError> {
        build_vless_split_http_over_relay(post_stream, get_stream, self.transport(), self.server())
            .await
    }
}

impl TcpStreamTransportPlan for OwnedVlessOutboundTransportPlan {
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

pub(in crate::vless_transport) struct VlessUdpOutboundTransportRequest<'a> {
    pub(super) socket: TokioSocket,
    pub(super) options: VlessUdpTransportOptions<'a>,
    pub(super) server: &'a str,
    pub(super) port: u16,
}
