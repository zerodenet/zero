//! Unified VLESS outbound transport builder.
//!
//! Wraps a raw TCP socket with the configured VLESS transport layer
//! (TLS / Reality / WebSocket / gRPC / H2), dispatching to the correct
//! connect function for every valid combination.

use std::path::Path;

use zero_config::{
    ClientTlsConfig, GrpcConfig, H2Config, HttpUpgradeConfig, RealityConfig, SplitHttpConfig,
    WebSocketConfig,
};
use zero_engine::EngineError;
use zero_platform_tokio::{RelayCarrier, TcpRelayStream, TokioSocket};

use std::io;

use zero_platform_tokio::TransportConnector;

use crate::{grpc, h2, http_upgrade, split_http, tls, ws};
use vless::{upgrade_reality_client, RealityClientOptions};

/// Wrap a raw TCP socket with the configured VLESS transport layer.
///
/// Handles every valid combination of TLS, Reality, WebSocket, gRPC, and H2.
/// Pass `None` for transports that are not configured.
pub async fn build_vless_outbound_transport(
    socket: TokioSocket,
    tls_config: Option<&ClientTlsConfig>,
    reality: Option<&RealityConfig>,
    ws_config: Option<&WebSocketConfig>,
    grpc_config: Option<&GrpcConfig>,
    h2_config: Option<&H2Config>,
    http_upgrade_config: Option<&HttpUpgradeConfig>,
    split_http_config: Option<&SplitHttpConfig>,
    source_dir: Option<&Path>,
    server: &str,
    port: u16,
) -> Result<TcpRelayStream, EngineError> {
    // SplitHTTP is handled first because it is mutually exclusive with other transports.
    if let Some(cfg) = split_http_config {
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

    // HTTP Upgrade is mutually exclusive with WS/gRPC/H2.
    if let Some(cfg) = http_upgrade_config {
        let stream: TcpRelayStream = match tls_config {
            Some(tls) => {
                let tls_stream = tls::connect_tls_upstream(socket, tls, source_dir, server).await?;
                TcpRelayStream::new(http_upgrade::connect_http_upgrade(tls_stream, cfg).await?)
            }
            None => TcpRelayStream::new(http_upgrade::connect_http_upgrade(socket, cfg).await?),
        };
        return Ok(stream);
    }

    match (tls_config, reality, ws_config, grpc_config, h2_config) {
        // gRPC
        (Some(tls), None, None, Some(grpc), None) => {
            let tls_stream = tls::connect_tls_upstream(socket, tls, source_dir, server).await?;
            let grpc_stream = grpc::connect_grpc(tls_stream, &grpc.service_names).await?;
            Ok(TcpRelayStream::new(grpc_stream))
        }
        (None, None, None, Some(grpc), None) => {
            let grpc_stream = grpc::connect_grpc(socket, &grpc.service_names).await?;
            Ok(TcpRelayStream::new(grpc_stream))
        }

        // H2
        (Some(tls), None, None, None, Some(h2_config)) => {
            let tls_stream = tls::connect_tls_upstream(socket, tls, source_dir, server).await?;
            let h2_stream = h2::connect_h2(tls_stream, h2_config, server, port).await?;
            Ok(TcpRelayStream::new(h2_stream))
        }
        (None, None, None, None, Some(h2_config)) => {
            let h2_stream = h2::connect_h2(socket, h2_config, server, port).await?;
            Ok(TcpRelayStream::new(h2_stream))
        }

        // WebSocket
        (Some(tls), None, Some(ws), None, None) => {
            let tls_stream = tls::connect_tls_upstream(socket, tls, source_dir, server).await?;
            let ws_stream = ws::connect_ws(tls_stream, ws, server, port).await?;
            Ok(TcpRelayStream::new(ws_stream))
        }
        (None, None, Some(ws), None, None) => {
            let ws_stream = ws::connect_ws(socket, ws, server, port).await?;
            Ok(TcpRelayStream::new(ws_stream))
        }

        // TLS only
        (Some(tls), None, None, None, None) => {
            tls::connect_tls_upstream(socket, tls, source_dir, server).await
        }

        // Reality
        (None, Some(reality), None, None, None) => {
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

        // Raw TCP
        (None, None, None, None, None) => Ok(socket.into()),

        _ => Err(EngineError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "invalid vless outbound transport combination",
        ))),
    }
}

// TransportConnector impl

/// Wrap an already established relay stream with the configured VLESS transport layer.
///
/// This is used after a relay prefix has connected to the final hop server.
/// Transports that need a second connection or a non-TCP carrier are rejected
/// here instead of being emulated in the proxy runtime.
pub async fn build_vless_outbound_transport_over_stream(
    carrier: RelayCarrier,
    tls_config: Option<&ClientTlsConfig>,
    reality: Option<&RealityConfig>,
    ws_config: Option<&WebSocketConfig>,
    grpc_config: Option<&GrpcConfig>,
    h2_config: Option<&H2Config>,
    http_upgrade_config: Option<&HttpUpgradeConfig>,
    split_http_config: Option<&SplitHttpConfig>,
    source_dir: Option<&Path>,
) -> Result<TcpRelayStream, EngineError> {
    let RelayCarrier {
        stream,
        server,
        port,
    } = carrier;
    // Shadow to match the original &str / u16 types used throughout the function.
    let server: &str = &server;
    if split_http_config.is_some() {
        return Err(EngineError::Io(io::Error::new(
            io::ErrorKind::Unsupported,
            "split_http final hop over relay stream is not supported",
        )));
    }

    if let Some(cfg) = http_upgrade_config {
        let stream: TcpRelayStream = match tls_config {
            Some(tls) => {
                let tls_stream = tls::connect_tls_stream(stream, tls, source_dir, server).await?;
                TcpRelayStream::new(http_upgrade::connect_http_upgrade(tls_stream, cfg).await?)
            }
            None => TcpRelayStream::new(http_upgrade::connect_http_upgrade(stream, cfg).await?),
        };
        return Ok(stream);
    }

    match (tls_config, reality, ws_config, grpc_config, h2_config) {
        (Some(tls), None, None, Some(grpc), None) => {
            let tls_stream = tls::connect_tls_stream(stream, tls, source_dir, server).await?;
            let grpc_stream = grpc::connect_grpc(tls_stream, &grpc.service_names).await?;
            Ok(TcpRelayStream::new(grpc_stream))
        }
        (None, None, None, Some(grpc), None) => {
            let grpc_stream = grpc::connect_grpc(stream, &grpc.service_names).await?;
            Ok(TcpRelayStream::new(grpc_stream))
        }
        (Some(tls), None, None, None, Some(h2_config)) => {
            let tls_stream = tls::connect_tls_stream(stream, tls, source_dir, server).await?;
            let h2_stream = h2::connect_h2(tls_stream, h2_config, server, port).await?;
            Ok(TcpRelayStream::new(h2_stream))
        }
        (None, None, None, None, Some(h2_config)) => {
            let h2_stream = h2::connect_h2(stream, h2_config, server, port).await?;
            Ok(TcpRelayStream::new(h2_stream))
        }
        (Some(tls), None, Some(ws), None, None) => {
            let tls_stream = tls::connect_tls_stream(stream, tls, source_dir, server).await?;
            let ws_stream = ws::connect_ws(tls_stream, ws, server, port).await?;
            Ok(TcpRelayStream::new(ws_stream))
        }
        (None, None, Some(ws), None, None) => {
            let ws_stream = ws::connect_ws(stream, ws, server, port).await?;
            Ok(TcpRelayStream::new(ws_stream))
        }
        (Some(tls), None, None, None, None) => {
            tls::connect_tls_stream(stream, tls, source_dir, server).await
        }
        (None, Some(reality), None, None, None) => {
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
        (None, None, None, None, None) => Ok(TcpRelayStream::new(stream)),
        _ => Err(EngineError::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid vless final-hop transport combination",
        ))),
    }
}

/// VLESS transport connector that implements [`TransportConnector`].
///
/// Created with transport configuration, then [`connect`] wraps each
/// raw socket with the configured transport layer.
///
/// [`connect`]: TransportConnector::connect
pub struct VlessTransportConnector<'a> {
    tls: Option<&'a ClientTlsConfig>,
    reality: Option<&'a RealityConfig>,
    ws: Option<&'a WebSocketConfig>,
    grpc: Option<&'a GrpcConfig>,
    h2: Option<&'a H2Config>,
    http_upgrade: Option<&'a HttpUpgradeConfig>,
    split_http: Option<&'a SplitHttpConfig>,
    source_dir: Option<&'a Path>,
}

impl<'a> VlessTransportConnector<'a> {
    /// Create a new connector with the given transport configuration.
    pub fn new(
        tls: Option<&'a ClientTlsConfig>,
        reality: Option<&'a RealityConfig>,
        ws: Option<&'a WebSocketConfig>,
        grpc: Option<&'a GrpcConfig>,
        h2: Option<&'a H2Config>,
        http_upgrade: Option<&'a HttpUpgradeConfig>,
        split_http: Option<&'a SplitHttpConfig>,
        source_dir: Option<&'a Path>,
    ) -> Self {
        Self {
            tls,
            reality,
            ws,
            grpc,
            h2,
            http_upgrade,
            split_http,
            source_dir,
        }
    }
}

impl TransportConnector for VlessTransportConnector<'_> {
    type Stream = TcpRelayStream;

    async fn connect(
        &self,
        socket: TokioSocket,
        server: &str,
        port: u16,
    ) -> io::Result<Self::Stream> {
        build_vless_outbound_transport(
            socket,
            self.tls,
            self.reality,
            self.ws,
            self.grpc,
            self.h2,
            self.http_upgrade,
            self.split_http,
            self.source_dir,
            server,
            port,
        )
        .await
        .map_err(|e| match e {
            EngineError::Io(io_err) => io_err,
            other => io::Error::other(other),
        })
    }
}
