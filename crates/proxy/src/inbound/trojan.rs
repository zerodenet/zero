//! Trojan inbound — TLS accept, protocol auth, route, TCP relay.

use std::io;

use tokio::select;
use tokio::sync::watch;
use tokio::task::JoinSet;
use tracing::{error, info, warn};
use zero_config::InboundConfig;
use zero_core::Session;
use zero_engine::EngineError;
use zero_protocol_trojan::TrojanInbound;
use zero_traits::AsyncSocket;

use crate::runtime::{bind_listener, Proxy};
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

impl Proxy {
    pub(crate) async fn run_trojan_listener(
        &self,
        inbound: InboundConfig,
        mut shutdown: watch::Receiver<bool>,
    ) -> Result<(), EngineError> {
        let (password, tls_cfg) = match &inbound.protocol {
            zero_config::InboundProtocolConfig::Trojan { password, sni: _, tls } => {
                (password.clone(), tls.clone())
            }
            _ => return Err(EngineError::Io(io::Error::new(io::ErrorKind::InvalidInput, "trojan config"))),
        };
        let tls_cfg = tls_cfg
            .ok_or_else(|| EngineError::Io(io::Error::new(io::ErrorKind::InvalidInput, "trojan requires TLS")))?;
        let acceptor = crate::transport::build_tls_acceptor(&tls_cfg, self.config.source_dir())?;
        let listener = bind_listener(&inbound).await?;
        let tag = inbound.tag.clone();

        info!(inbound_tag = %tag, protocol = "trojan", listen = %listener.local_addr()?, "started");

        let mut conns = JoinSet::new();
        loop {
            select! {
                _ = shutdown.changed() => { if *shutdown.borrow() { break; } }
                r = listener.accept() => {
                    let (s, peer) = match r { Ok(v) => v, Err(e) => { error!(%e, "accept"); continue; } };
                    let p = self.clone(); let t = tag.clone(); let pw = password.clone(); let a = acceptor.clone();
                    conns.spawn(async move {
                        if let Err(e) = p.serve_trojan(s, &t, &pw, &a).await {
                            if !matches!(&e, EngineError::Io(io) if matches!(io.kind(),
                                io::ErrorKind::UnexpectedEof | io::ErrorKind::ConnectionReset | io::ErrorKind::BrokenPipe))
                            { warn!(?peer, %e, "trojan failed"); }
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

    async fn serve_trojan(
        &self, socket: impl Into<TcpRelayStream>, tag: &str, password: &str,
        acceptor: &crate::transport::TlsAcceptor,
    ) -> Result<(), EngineError> {
        let raw: TcpRelayStream = socket.into();
        // TLS accept.
        let tls = acceptor.accept(raw).await
            .map_err(|e| EngineError::Io(io::Error::new(io::ErrorKind::Other, e)))?;
        // Trojan protocol auth.
        let mut sock = TlsStream(tls);
        let accept = TrojanInbound.accept(&mut sock, &[password.to_owned()]).await?;
        let mut session: Session = accept.session;
        self.prepare_session(&mut session, tag);
        // Route + outbound + relay.
        let action = self.route_decision(&session.target);
        let (resolved, _plan) = self.resolve_outbound(&action)?;
        let outbound = self.establish_tcp_outbound(&session, (resolved, _plan)).await
            .map_err(|f| EngineError::Io(io::Error::new(io::ErrorKind::Other, f.error)))?;
        let upstream = match outbound {
            crate::transport::EstablishedTcpOutbound::Direct { upstream, .. } => upstream,
            crate::transport::EstablishedTcpOutbound::Vless { upstream, .. } => upstream,
            crate::transport::EstablishedTcpOutbound::Socks5 { upstream, .. } => upstream,
            crate::transport::EstablishedTcpOutbound::Hysteria2 { upstream, .. } => upstream,
            crate::transport::EstablishedTcpOutbound::Shadowsocks { upstream, .. } => upstream,
            crate::transport::EstablishedTcpOutbound::Trojan { upstream, .. } => upstream,
            crate::transport::EstablishedTcpOutbound::Relay { upstream } => upstream,
            crate::transport::EstablishedTcpOutbound::Block { .. } => {
                return Err(EngineError::Io(io::Error::new(io::ErrorKind::ConnectionRefused, "blocked")));
            }
        };
        crate::transport::relay_bidirectional_metered(
            TcpRelayStream::new(sock.0), upstream, |_| {}, |_| {},
        ).await
        .map_err(|e| EngineError::Io(e))?;
        Ok(())
    }
}
