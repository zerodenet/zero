//! Trojan inbound — TLS accept, protocol auth, route, TCP relay.

use std::io;

use async_trait::async_trait;
use tokio::select;
use tokio::sync::watch;
use tokio::task::JoinSet;
use tracing::{error, info, warn};
use trojan::TrojanInbound;
use zero_config::InboundConfig;
use zero_core::Session;
use zero_engine::EngineError;
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

// ── Trait-based handler ────────────────────────────────────────────────

#[derive(Clone)]
pub(crate) struct TrojanInboundHandler {
    trojan_inbound: TrojanInbound,
    password: String,
    tls_acceptor: crate::transport::TlsAcceptor,
}

#[async_trait]
impl InboundProtocol for TrojanInboundHandler {
    type ClientStream = TcpRelayStream;

    async fn accept(
        &self,
        stream: TcpRelayStream,
    ) -> Result<(Session, Self::ClientStream), EngineError> {
        // TLS accept
        let tls = self
            .tls_acceptor
            .accept(stream)
            .await
            .map_err(|e| EngineError::Io(io::Error::other(e)))?;
        // Trojan protocol auth
        let mut sock = TlsStream(tls);
        let accept = self
            .trojan_inbound
            .accept(&mut sock, &[self.password.clone()])
            .await?;
        let mut session: Session = accept.session;
        let mut sa = zero_core::SessionAuth::new("trojan");
        sa.principal_key = Some(self.password.clone());
        session.apply_auth(sa);
        Ok((session, TcpRelayStream::new(sock.0)))
    }

    async fn send_ok(&self, _client: &mut TcpRelayStream) -> Result<(), EngineError> {
        // Trojan has no success response
        Ok(())
    }

    async fn send_blocked(&self, _client: &mut TcpRelayStream) -> Result<(), EngineError> {
        // Trojan has no blocked response — just close
        Ok(())
    }

    async fn send_upstream_failure(&self, _client: &mut TcpRelayStream) -> Result<(), EngineError> {
        Ok(())
    }
    // relay uses default
}

// ── Listener ────────────────────────────────────────────────────────────

impl Proxy {
    pub(crate) async fn run_trojan_listener(
        &self,
        inbound: InboundConfig,
        mut shutdown: watch::Receiver<bool>,
    ) -> Result<(), EngineError> {
        let (password, tls_cfg, _up_bps, _down_bps) = match &inbound.protocol {
            zero_config::InboundProtocolConfig::Trojan {
                password,
                sni: _,
                tls,
                up_bps,
                down_bps,
            } => (password.clone(), tls.clone(), *up_bps, *down_bps),
            _ => {
                return Err(EngineError::Io(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "trojan config",
                )))
            }
        };
        let tls_cfg = tls_cfg.ok_or_else(|| {
            EngineError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "trojan requires TLS",
            ))
        })?;
        let acceptor = crate::transport::build_tls_acceptor(&tls_cfg, self.config.source_dir())?;
        let listener = bind_listener(&inbound).await?;
        let tag = inbound.tag.clone();

        let handler = TrojanInboundHandler {
            trojan_inbound: TrojanInbound,
            password,
            tls_acceptor: acceptor,
        };

        info!(inbound_tag = %tag, protocol = "trojan", listen = %listener.local_addr()?, "started");

        let mut conns = JoinSet::new();
        loop {
            select! {
                _ = shutdown.changed() => { if *shutdown.borrow() { break; } }
                r = listener.accept() => {
                    let (s, peer) = match r { Ok(v) => v, Err(e) => { error!(%e, "accept"); continue; } };
                    let p = self.clone();
                    let t = tag.clone();
                    let h = handler.clone();
                    let source_addr = remote_addr_to_socket(peer);
                    conns.spawn(async move {
                        match h.accept(s.into()).await {
                            Ok((session, client)) => {
                                if let Err(e) = serve_inbound(
                                    &p, session, client, &h, &t, source_addr,
                                ).await {
                                    if !matches!(&e, EngineError::Io(io) if matches!(io.kind(),
                                        io::ErrorKind::UnexpectedEof | io::ErrorKind::ConnectionReset | io::ErrorKind::BrokenPipe))
                                    { warn!(?source_addr, %e, "trojan failed"); }
                                }
                            }
                            Err(e) => {
                                if !matches!(&e, EngineError::Io(io) if matches!(io.kind(),
                                    io::ErrorKind::UnexpectedEof | io::ErrorKind::ConnectionReset | io::ErrorKind::BrokenPipe))
                                { warn!(?source_addr, %e, "trojan auth failed"); }
                            }
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

fn remote_addr_to_socket(addr: Option<zero_traits::IpAddress>) -> Option<std::net::SocketAddr> {
    addr.map(|ip| match ip {
        zero_traits::IpAddress::V4(octets) => {
            std::net::SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::from(octets)), 0)
        }
        zero_traits::IpAddress::V6(octets) => {
            std::net::SocketAddr::new(std::net::IpAddr::V6(std::net::Ipv6Addr::from(octets)), 0)
        }
    })
}
