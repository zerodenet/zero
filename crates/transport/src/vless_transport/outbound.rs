use std::future::Future;
use std::io;
use std::path::{Path, PathBuf};

use zero_config::{
    ClientTlsConfig, GrpcConfig, H2Config, HttpUpgradeConfig, QuicConfig, RealityConfig,
    SplitHttpConfig, WebSocketConfig,
};
use zero_engine::EngineError;
use zero_platform_tokio::{RelayCarrier, TcpRelayStream, TokioSocket};
use zero_traits::StreamMuxTransportHints;

use crate::outbound_stack::{
    connect_relay_transport_stack, connect_socket_transport_stack, StreamTransportStack,
};
use crate::transport_plan::TcpStreamTransportPlan;
use crate::{quic, split_http, tls};
use vless::reality::{upgrade_reality_client, RealityClientOptions};
#[derive(Clone, Copy)]
pub(super) struct VlessTransportOptions<'a> {
    tls: Option<&'a ClientTlsConfig>,
    reality: Option<&'a RealityConfig>,
    ws: Option<&'a WebSocketConfig>,
    grpc: Option<&'a GrpcConfig>,
    h2: Option<&'a H2Config>,
    http_upgrade: Option<&'a HttpUpgradeConfig>,
    split_http: Option<&'a SplitHttpConfig>,
    source_dir: Option<&'a Path>,
}

impl<'a> VlessTransportOptions<'a> {
    pub(super) fn uses_deferred_tcp_response(self) -> bool {
        self.reality.is_some()
    }
}

pub(super) struct VlessOutboundTransportRequest<'a> {
    pub(super) socket: TokioSocket,
    pub(super) options: VlessTransportOptions<'a>,
    pub(super) server: &'a str,
    pub(super) port: u16,
}

struct VlessDirectTransportRequest<'a> {
    socket: Option<TokioSocket>,
    options: VlessTransportOptions<'a>,
    quic: Option<&'a QuicConfig>,
    server: &'a str,
    port: u16,
}

struct VlessFinalHopTransportRequest<'a> {
    carrier: RelayCarrier,
    options: VlessTransportOptions<'a>,
}

#[derive(Clone, Copy)]
pub(super) struct VlessUdpTransportOptions<'a> {
    tls: Option<&'a ClientTlsConfig>,
    reality: Option<&'a RealityConfig>,
    ws: Option<&'a WebSocketConfig>,
    grpc: Option<&'a GrpcConfig>,
    h2: Option<&'a H2Config>,
    http_upgrade: Option<&'a HttpUpgradeConfig>,
    split_http: Option<&'a SplitHttpConfig>,
    quic: Option<&'a QuicConfig>,
    source_dir: Option<&'a Path>,
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

    pub(super) fn stream_options(self) -> VlessTransportOptions<'a> {
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
pub(super) struct OwnedVlessOutboundTransportPlan {
    server: String,
    port: u16,
    transport: OwnedVlessUdpTransportOptions,
}

impl OwnedVlessOutboundTransportPlan {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn from_config_refs(
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

    pub(super) fn server(&self) -> &str {
        &self.server
    }

    pub(super) fn port(&self) -> u16 {
        self.port
    }

    pub(super) fn transport(&self) -> VlessUdpTransportOptions<'_> {
        self.transport.as_borrowed()
    }

    pub(super) fn stream_transport_options(&self) -> VlessTransportOptions<'_> {
        self.transport.stream_options()
    }

    pub(super) fn uses_deferred_tcp_response(&self) -> bool {
        self.stream_transport_options().uses_deferred_tcp_response()
    }

    pub(super) fn uses_quic(&self) -> bool {
        self.transport().quic.is_some()
    }

    pub(super) fn relay_needs_two_streams(&self) -> bool {
        self.transport().uses_paired_relay_transport()
    }

    pub(super) fn mux_transport_hints(&self) -> StreamMuxTransportHints {
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

    pub(super) async fn open_direct<OpenSocket, OpenSocketFut, E>(
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

    pub(super) async fn open_relay(
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

    pub(super) async fn build_relay_two_stream_udp_transport(
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

pub(super) struct VlessUdpOutboundTransportRequest<'a> {
    pub(super) socket: TokioSocket,
    pub(super) options: VlessUdpTransportOptions<'a>,
    pub(super) server: &'a str,
    pub(super) port: u16,
}

async fn open_vless_quic_transport(
    server: &str,
    port: u16,
    quic_config: &QuicConfig,
) -> Result<TcpRelayStream, EngineError> {
    let server_name = quic_config.server_name.as_deref().unwrap_or(server);
    Ok(TcpRelayStream::new(
        quic::connect_quic(server_name, port, quic_config.insecure).await?,
    ))
}

async fn build_vless_direct_outbound_transport(
    request: VlessDirectTransportRequest<'_>,
) -> Result<TcpRelayStream, EngineError> {
    let VlessDirectTransportRequest {
        socket,
        options,
        quic,
        server,
        port,
    } = request;

    if let Some(quic_config) = quic {
        return open_vless_quic_transport(server, port, quic_config).await;
    }

    let socket = socket.ok_or_else(|| {
        EngineError::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            "missing tcp socket for direct VLESS outbound transport",
        ))
    })?;

    build_vless_outbound_transport(VlessOutboundTransportRequest {
        socket,
        options,
        server,
        port,
    })
    .await
}

/// Wrap a raw TCP socket with the configured VLESS transport layer.
///
/// Handles every valid combination of TLS, Reality, WebSocket, gRPC, and H2.
/// Pass `None` for transports that are not configured.
pub(super) async fn build_vless_outbound_transport(
    request: VlessOutboundTransportRequest<'_>,
) -> Result<TcpRelayStream, EngineError> {
    let VlessOutboundTransportRequest {
        socket,
        options,
        server,
        port,
    } = request;
    let VlessTransportOptions {
        tls: tls_config,
        reality,
        ws: ws_config,
        grpc: grpc_config,
        h2: h2_config,
        http_upgrade: http_upgrade_config,
        split_http: split_http_config,
        source_dir,
    } = options;

    // XHTTP is handled first because it is mutually exclusive with other transports.
    if let Some(cfg) = split_http_config {
        let mode = split_http::XhttpMode::parse(&cfg.mode);
        // stream-one (and `auto`): a single bidirectional connection — one TCP/TLS
        // socket carries both the chunked upload and the chunked download.
        if mode.is_single_connection() {
            let carrier: TcpRelayStream = match tls_config {
                Some(tls) => tls::connect_tls_upstream(socket, tls, source_dir, server).await?,
                None => TcpRelayStream::new(socket),
            };
            return Ok(TcpRelayStream::new(
                split_http::connect_xhttp_stream_one(carrier, cfg).await?,
            ));
        }

        // packet-up / stream-up: legacy two-connection model (POST + GET).
        let peer = socket.peer_addr().map_err(EngineError::Io)?;
        let stream: TcpRelayStream = match tls_config {
            Some(tls) => {
                let post_stream =
                    tls::connect_tls_upstream(socket, tls, source_dir, server).await?;
                match TokioSocket::connect_addr(peer).await {
                    Ok(get_socket) => {
                        let get_stream =
                            match tls::connect_tls_upstream(get_socket, tls, source_dir, server)
                                .await
                            {
                                Ok(s) => s,
                                Err(e) => {
                                    // GET TLS connect failed; drop the POST stream.
                                    drop(post_stream);
                                    return Err(e);
                                }
                            };
                        TcpRelayStream::new(
                            split_http::connect_split_http(post_stream, get_stream, cfg).await?,
                        )
                    }
                    Err(e) => {
                        // Cannot open GET TCP; release the POST stream.
                        drop(post_stream);
                        return Err(EngineError::Io(io::Error::new(
                            io::ErrorKind::ConnectionRefused,
                            format!("split-http: failed to open GET connection: {e}"),
                        )));
                    }
                }
            }
            None => {
                let get_socket = TokioSocket::connect_addr(peer).await.map_err(|e| {
                    EngineError::Io(io::Error::new(
                        io::ErrorKind::ConnectionRefused,
                        format!("split-http: failed to open GET connection: {e}"),
                    ))
                })?;
                TcpRelayStream::new(split_http::connect_split_http(socket, get_socket, cfg).await?)
            }
        };
        return Ok(stream);
    }

    if let Some(reality) = reality {
        return match (
            tls_config,
            ws_config,
            grpc_config,
            h2_config,
            http_upgrade_config,
        ) {
            (None, None, None, None, None) => {
                let server_name = reality.server_name.as_deref().unwrap_or(server);
                let reality_stream = upgrade_reality_client(
                    socket,
                    RealityClientOptions {
                        public_key: &reality.public_key,
                        short_id: &reality.short_id,
                        server_name,
                        cipher_suites: &reality.cipher_suites,
                    },
                )
                .await?;
                Ok(TcpRelayStream::new(reality_stream))
            }
            _ => Err(EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "invalid vless outbound transport combination",
            ))),
        };
    }

    connect_socket_transport_stack(
        socket,
        StreamTransportStack {
            tls: tls_config,
            ws: ws_config,
            grpc: grpc_config,
            h2: h2_config,
            http_upgrade: http_upgrade_config,
            source_dir,
        },
        server,
        port,
        "invalid vless outbound transport combination",
    )
    .await
}

pub(super) async fn build_vless_udp_outbound_transport(
    request: VlessUdpOutboundTransportRequest<'_>,
) -> Result<TcpRelayStream, EngineError> {
    let VlessUdpOutboundTransportRequest {
        socket,
        options,
        server,
        port,
    } = request;

    if let Some(quic_config) = options.quic {
        return open_vless_quic_transport(server, port, quic_config).await;
    }

    build_vless_outbound_transport(VlessOutboundTransportRequest {
        socket,
        options: options.stream_options(),
        server,
        port,
    })
    .await
}

// TransportConnector impl

/// Wrap an already established relay stream with the configured VLESS transport layer.
///
/// This is used after a relay prefix has connected to the final hop server.
/// Transports that need a second connection or a non-TCP carrier are rejected
/// here instead of being emulated in the proxy runtime.
async fn build_vless_outbound_transport_over_stream(
    request: VlessFinalHopTransportRequest<'_>,
) -> Result<TcpRelayStream, EngineError> {
    let VlessFinalHopTransportRequest { carrier, options } = request;
    let RelayCarrier {
        stream,
        server,
        port,
    } = carrier;
    // Shadow to match the original &str / u16 types used throughout the function.
    let server: &str = &server;
    let VlessTransportOptions {
        tls: tls_config,
        reality,
        ws: ws_config,
        grpc: grpc_config,
        h2: h2_config,
        http_upgrade: http_upgrade_config,
        split_http: split_http_config,
        source_dir,
    } = options;

    if let Some(cfg) = split_http_config {
        let mode = split_http::XhttpMode::parse(&cfg.mode);
        // stream-one (and `auto`): a single bidirectional connection. This is
        // the path that makes XHTTP usable as a relay-chain final hop — the
        // relay prefix delivers exactly one stream, which stream-one uses for
        // both the chunked upload and the chunked download.
        if mode.is_single_connection() {
            let carrier: TcpRelayStream = match tls_config {
                Some(tls) => tls::connect_tls_stream(stream, tls, source_dir, server).await?,
                None => stream,
            };
            return Ok(TcpRelayStream::new(
                split_http::connect_xhttp_stream_one(carrier, cfg).await?,
            ));
        }
        // packet-up / stream-up require two independent connections (POST + GET);
        // a relay chain provides only one stream per hop, so they cannot serve
        // as a final hop here. The two-stream path lives in
        // `build_vless_split_http_over_relay` (used by the UDP relay fast path).
        return Err(EngineError::Io(io::Error::new(
            io::ErrorKind::Unsupported,
            "xhttp packet-up/stream-up require two streams; use mode stream-one (or auto) for relay final-hop",
        )));
    }

    if let Some(reality) = reality {
        return match (
            tls_config,
            ws_config,
            grpc_config,
            h2_config,
            http_upgrade_config,
        ) {
            (None, None, None, None, None) => {
                let server_name = reality.server_name.as_deref().unwrap_or(server);
                let reality_stream = upgrade_reality_client(
                    stream,
                    RealityClientOptions {
                        public_key: &reality.public_key,
                        short_id: &reality.short_id,
                        server_name,
                        cipher_suites: &reality.cipher_suites,
                    },
                )
                .await?;
                Ok(TcpRelayStream::new(reality_stream))
            }
            _ => Err(EngineError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid vless final-hop transport combination",
            ))),
        };
    }

    connect_relay_transport_stack(
        stream,
        StreamTransportStack {
            tls: tls_config,
            ws: ws_config,
            grpc: grpc_config,
            h2: h2_config,
            http_upgrade: http_upgrade_config,
            source_dir,
        },
        server,
        port,
        "invalid vless final-hop transport combination",
    )
    .await
}

/// Build a SplitHTTP transport over a relay chain.
///
/// SplitHTTP uses separate POST (write) and GET (read) TCP channels.
/// This function receives two independent streams — each already tunneled
/// through the same relay prefix hops — and pairs them via
/// [`split_http::connect_split_http`].
async fn build_vless_split_http_over_relay(
    post_stream: TcpRelayStream,
    get_stream: TcpRelayStream,
    options: VlessUdpTransportOptions<'_>,
    server: &str,
) -> Result<TcpRelayStream, EngineError> {
    let VlessUdpTransportOptions {
        tls: tls_config,
        split_http: split_http_config,
        source_dir,
        ..
    } = options;
    let config = split_http_config.ok_or_else(|| {
        EngineError::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            "split-http relay transport requires split_http config",
        ))
    })?;

    if split_http::XhttpMode::parse(&config.mode).is_single_connection() {
        return Err(EngineError::Io(io::Error::new(
            io::ErrorKind::Unsupported,
            "split-http relay transport requires packet-up or stream-up mode",
        )));
    }

    let post_stream = match tls_config {
        Some(tls) => tls::connect_tls_stream(post_stream, tls, source_dir, server).await?,
        None => post_stream,
    };
    let get_stream = match tls_config {
        Some(tls) => tls::connect_tls_stream(get_stream, tls, source_dir, server).await?,
        None => get_stream,
    };
    let paired = split_http::connect_split_http(post_stream, get_stream, config).await?;
    Ok(TcpRelayStream::new(paired))
}
