//! VMess inbound — TLS accept, transport dispatch (WS/gRPC), protocol auth, route, TCP relay.

use std::io;

use async_trait::async_trait;
use tokio::select;
use tokio::sync::watch;
use tokio::task::JoinSet;
use tracing::{error, info, warn};
use zero_config::{GrpcConfig, InboundConfig, WebSocketConfig};
use zero_core::Session;
use zero_engine::EngineError;
use zero_protocol_vmess::{VmessCipher, VmessInbound, VmessUser};
use zero_traits::AsyncSocket;

use crate::runtime::bind_listener;
use crate::runtime::inbound_protocol::{serve_inbound, InboundProtocol};
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;

/// `AsyncSocket` for a rustls TLS stream over TcpRelayStream.
struct TlsStream(tokio_rustls::server::TlsStream<TcpRelayStream>);

impl AsyncSocket for TlsStream {
    type Error = io::Error;
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        tokio::io::AsyncReadExt::read(&mut self.0, buf).await
    }
    async fn write_all(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
        tokio::io::AsyncWriteExt::write_all(&mut self.0, buf).await
    }
    async fn shutdown(&mut self) -> Result<(), Self::Error> {
        tokio::io::AsyncWriteExt::shutdown(&mut self.0).await
    }
}

// ── Trait-based handler (raw TLS path) ────────────────────────────────────

#[derive(Clone)]
pub(crate) struct VmessInboundHandler {
    vmess_inbound: VmessInbound,
    users: Vec<VmessUser>,
    tls_acceptor: crate::transport::TlsAcceptor,
}

#[async_trait]
impl InboundProtocol for VmessInboundHandler {
    type ClientStream = TcpRelayStream;

    async fn accept(
        &self,
        stream: TcpRelayStream,
    ) -> Result<(Session, Self::ClientStream), EngineError> {
        let tls = self
            .tls_acceptor
            .accept(stream)
            .await
            .map_err(|e| EngineError::Io(io::Error::new(io::ErrorKind::Other, e)))?;
        let mut sock = TlsStream(tls);
        let session = if self.users.len() == 1 {
            self.vmess_inbound
                .accept_tcp_with_auth(&mut sock, &self.users[0])
                .await?
        } else {
            self.vmess_inbound
                .accept_tcp_with_auth_multi(&mut sock, &self.users)
                .await?
        };
        Ok((session, TcpRelayStream::new(sock.0)))
    }

    async fn send_ok(&self, _client: &mut TcpRelayStream) -> Result<(), EngineError> {
        Ok(())
    }

    async fn send_blocked(&self, _client: &mut TcpRelayStream) -> Result<(), EngineError> {
        Ok(())
    }

    async fn send_upstream_failure(&self, _client: &mut TcpRelayStream) -> Result<(), EngineError> {
        Ok(())
    }
}

// ── Handler for transport-wrapped connections (WS/gRPC) ────────────────────
// Only send_ok / send_blocked / send_upstream_failure are used by serve_inbound;
// accept is unreachable because the protocol was already authenticated.

#[derive(Clone)]
struct VmessTransportHandler;

#[async_trait]
impl InboundProtocol for VmessTransportHandler {
    type ClientStream = TcpRelayStream;

    async fn accept(
        &self,
        _stream: TcpRelayStream,
    ) -> Result<(Session, Self::ClientStream), EngineError> {
        unreachable!("accept handled in listener transport dispatch")
    }

    async fn send_ok(&self, _client: &mut TcpRelayStream) -> Result<(), EngineError> {
        Ok(())
    }

    async fn send_blocked(&self, _client: &mut TcpRelayStream) -> Result<(), EngineError> {
        Ok(())
    }

    async fn send_upstream_failure(&self, _client: &mut TcpRelayStream) -> Result<(), EngineError> {
        Ok(())
    }
}

// ── Listener ──────────────────────────────────────────────────────────────

impl Proxy {
    pub(crate) async fn run_vmess_listener(
        &self,
        inbound: InboundConfig,
        mut shutdown: watch::Receiver<bool>,
    ) -> Result<(), EngineError> {
        let (users, tls_cfg, ws_config, grpc_config) = match &inbound.protocol {
            zero_config::InboundProtocolConfig::Vmess {
                users,
                tls,
                ws,
                grpc,
            } => (users.clone(), tls.clone(), ws.clone(), grpc.clone()),
            _ => {
                return Err(EngineError::Io(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "vmess config",
                )))
            }
        };
        if users.is_empty() {
            return Err(EngineError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "vmess requires at least one user",
            )));
        }

        let vmess_users: Vec<VmessUser> = users
            .iter()
            .map(|u| {
                let uuid = zero_protocol_vmess::parse_uuid(&u.id)
                    .map_err(|e| EngineError::Io(io::Error::new(io::ErrorKind::InvalidInput, e)))?;
                let cipher = VmessCipher::from_name(&u.cipher).ok_or_else(|| {
                    EngineError::Io(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("vmess unknown cipher: {}", u.cipher),
                    ))
                })?;
                Ok(VmessUser {
                    id: uuid,
                    cipher,
                    credential_id: u.credential_id.clone(),
                    principal_key: u.principal_key.clone(),
                    up_bps: u.up_bps,
                    down_bps: u.down_bps,
                })
            })
            .collect::<Result<Vec<_>, EngineError>>()?;

        let tls_cfg = tls_cfg.ok_or_else(|| {
            EngineError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "vmess requires TLS",
            ))
        })?;
        let acceptor = crate::transport::build_tls_acceptor(&tls_cfg, self.config.source_dir())?;
        let listener = bind_listener(&inbound).await?;
        let tag = inbound.tag.clone();

        let handler = VmessInboundHandler {
            vmess_inbound: VmessInbound,
            users: vmess_users,
            tls_acceptor: acceptor,
        };

        let transport = match (&ws_config, &grpc_config) {
            (Some(_), Some(_)) => {
                return Err(EngineError::Io(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "vmess: ws and grpc are mutually exclusive",
                )))
            }
            (Some(_), None) => "vmess+ws",
            (None, Some(_)) => "vmess+grpc",
            (None, None) => "vmess",
        };

        info!(inbound_tag = %tag, protocol = transport, listen = %listener.local_addr()?, "started");

        let mut conns = JoinSet::new();
        loop {
            select! {
                _ = shutdown.changed() => { if *shutdown.borrow() { break; } }
                r = listener.accept() => {
                    let (s, peer) = match r { Ok(v) => v, Err(e) => { error!(%e, "accept"); continue; } };
                    let p = self.clone();
                    let t = tag.clone();
                    let h = handler.clone();
                    let ws = ws_config.clone();
                    let grpc = grpc_config.clone();
                    let source_addr = remote_addr_to_socket(peer);
                    conns.spawn(async move {
                        let res = if let Some(grpc_cfg) = &grpc {
                            handle_vmess_grpc(&p, &h, s.into(), grpc_cfg, &t, source_addr).await
                        } else if let Some(ws_cfg) = &ws {
                            handle_vmess_ws(&p, &h, s.into(), ws_cfg, &t, source_addr).await
                        } else {
                            handle_vmess_raw(&p, &h, s.into(), &t, source_addr).await
                        };
                        if let Err(e) = res {
                            if !matches!(&e, EngineError::Io(io) if matches!(io.kind(),
                                io::ErrorKind::UnexpectedEof | io::ErrorKind::ConnectionReset | io::ErrorKind::BrokenPipe))
                            { warn!(?source_addr, %e, "vmess failed"); }
                        }
                    });
                }
                r = conns.join_next(), if !conns.is_empty() => {
                    if let Some(Err(e)) = r { if !e.is_cancelled() { error!(%e, "task panicked"); } }
                }
            }
        }
        conns.abort_all();
        info!(inbound_tag = %tag, "stopped");
        Ok(())
    }
}

/// Raw TLS path: TLS accept → VMess auth → serve_inbound.
async fn handle_vmess_raw(
    proxy: &Proxy,
    handler: &VmessInboundHandler,
    stream: TcpRelayStream,
    tag: &str,
    source_addr: Option<std::net::SocketAddr>,
) -> Result<(), EngineError> {
    match handler.accept(stream).await {
        Ok((session, client)) => {
            serve_inbound(proxy, session, client, handler, tag, source_addr).await
        }
        Err(e) => Err(e),
    }
}

/// WebSocket path: TLS accept → WS upgrade → VMess auth → serve_inbound.
async fn handle_vmess_ws(
    proxy: &Proxy,
    handler: &VmessInboundHandler,
    stream: TcpRelayStream,
    ws_cfg: &WebSocketConfig,
    tag: &str,
    source_addr: Option<std::net::SocketAddr>,
) -> Result<(), EngineError> {
    let tls = handler
        .tls_acceptor
        .accept(stream)
        .await
        .map_err(|e| EngineError::Io(io::Error::new(io::ErrorKind::Other, e)))?;

    let mut ws = crate::transport::accept_ws(tls, &ws_cfg.path).await?;

    let session = if handler.users.len() == 1 {
        handler
            .vmess_inbound
            .accept_tcp_with_auth(&mut ws, &handler.users[0])
            .await?
    } else {
        handler
            .vmess_inbound
            .accept_tcp_with_auth_multi(&mut ws, &handler.users)
            .await?
    };

    let transport_handler = VmessTransportHandler;
    serve_inbound(
        proxy,
        session,
        TcpRelayStream::new(ws),
        &transport_handler,
        tag,
        source_addr,
    )
    .await
}

/// gRPC path: TLS accept → serve_grpc → per-stream VMess auth → serve_inbound.
async fn handle_vmess_grpc(
    proxy: &Proxy,
    handler: &VmessInboundHandler,
    stream: TcpRelayStream,
    grpc_cfg: &GrpcConfig,
    tag: &str,
    source_addr: Option<std::net::SocketAddr>,
) -> Result<(), EngineError> {
    let tls = handler
        .tls_acceptor
        .accept(stream)
        .await
        .map_err(|e| EngineError::Io(io::Error::new(io::ErrorKind::Other, e)))?;

    let service_names = grpc_cfg.service_names.clone();
    let users = handler.users.clone();
    let vmess = handler.vmess_inbound;
    let proxy = proxy.clone();
    let tag = tag.to_owned();

    crate::transport::serve_grpc(tls, &service_names, move |mut grpc_stream| {
        let users = users.clone();
        let proxy = proxy.clone();
        let tag = tag.clone();
        async move {
            let result = if users.len() == 1 {
                vmess
                    .accept_tcp_with_auth(&mut grpc_stream, &users[0])
                    .await
            } else {
                vmess
                    .accept_tcp_with_auth_multi(&mut grpc_stream, &users)
                    .await
            };
            match result {
                Ok(session) => {
                    let transport_handler = VmessTransportHandler;
                    serve_inbound(
                        &proxy,
                        session,
                        TcpRelayStream::new(grpc_stream),
                        &transport_handler,
                        &tag,
                        source_addr,
                    )
                    .await
                }
                Err(e) => {
                    warn!(%e, "vmess grpc auth failed");
                    Err(EngineError::Core(zero_core::Error::Protocol(
                        "vmess auth failed",
                    )))
                }
            }
        }
    })
    .await
}

fn remote_addr_to_socket(addr: Option<zero_traits::IpAddress>) -> Option<std::net::SocketAddr> {
    addr.and_then(|ip| match ip {
        zero_traits::IpAddress::V4(octets) => Some(std::net::SocketAddr::new(
            std::net::IpAddr::V4(std::net::Ipv4Addr::from(octets)),
            0,
        )),
        zero_traits::IpAddress::V6(octets) => Some(std::net::SocketAddr::new(
            std::net::IpAddr::V6(std::net::Ipv6Addr::from(octets)),
            0,
        )),
    })
}
