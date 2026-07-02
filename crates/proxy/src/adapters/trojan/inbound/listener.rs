//! Trojan inbound: TLS accept, protocol auth, route, TCP/UDP relay.

mod udp;

use std::io;

use tokio::sync::watch;
use tracing::warn;
use trojan::{TrojanInbound, TrojanInboundProfile};
use zero_config::InboundConfig;
use zero_core::Session;
use zero_engine::EngineError;

use udp::run_trojan_udp_relay;

use crate::runtime::inbound_protocol::{serve_inbound, NoClientResponseInboundProtocol};
use crate::runtime::listener_loop::{run_tcp_listener_loop, TcpListenerLoopRequest};
use crate::runtime::Proxy;
use crate::transport::{AsyncSocketStream, TcpRelayStream};

// Listener.

pub(crate) async fn run_trojan_listener_with_bound(
    proxy: &Proxy,
    inbound: InboundConfig,
    profile: TrojanInboundProfile,
    tls_acceptor: crate::transport::TlsAcceptor,
    listener: zero_platform_tokio::TokioListener,
    shutdown: watch::Receiver<bool>,
) -> Result<(), EngineError> {
    run_tcp_listener_loop(TcpListenerLoopRequest {
        proxy,
        inbound_tag: inbound.tag,
        protocol_name: "trojan",
        listener,
        shutdown,
        handler: move |engine: Proxy,
                       tag: String,
                       stream: zero_platform_tokio::TokioSocket,
                       source_addr: Option<std::net::SocketAddr>| {
            let profile = profile.clone();
            let tls_acceptor = tls_acceptor.clone();
            async move {
                let tls = match tls_acceptor.accept(stream).await {
                    Ok(tls) => tls,
                    Err(e) => {
                        let e = EngineError::Io(io::Error::other(e));
                        if !matches!(&e, EngineError::Io(io) if matches!(io.kind(),
                            io::ErrorKind::UnexpectedEof | io::ErrorKind::ConnectionReset | io::ErrorKind::BrokenPipe))
                        {
                            warn!(?source_addr, %e, "trojan tls accept failed");
                        }
                        return;
                    }
                };
                let client = match profile
                    .accept_client(TrojanInbound, AsyncSocketStream::new(tls))
                    .await
                {
                    Ok(client) => client,
                    Err(e) => {
                        let e = EngineError::from(e);
                        if !matches!(&e, EngineError::Io(io) if matches!(io.kind(),
                            io::ErrorKind::UnexpectedEof | io::ErrorKind::ConnectionReset | io::ErrorKind::BrokenPipe))
                        {
                            warn!(?source_addr, %e, "trojan auth failed");
                        }
                        return;
                    }
                };

                let engine_ref = &engine;
                let tag_ref = &tag;
                let result = client
                    .dispatch(
                        |session: Session,
                         stream: AsyncSocketStream<
                            tokio_rustls::server::TlsStream<zero_platform_tokio::TokioSocket>,
                        >| async move {
                            serve_inbound(
                                engine_ref,
                                session,
                                TcpRelayStream::new(stream.into_inner()),
                                &NoClientResponseInboundProtocol,
                                tag_ref,
                                source_addr,
                            )
                            .await
                        },
                        |session: Session,
                         stream: AsyncSocketStream<
                            tokio_rustls::server::TlsStream<zero_platform_tokio::TokioSocket>,
                        >,
                         responder: trojan::udp::TrojanInboundUdpResponder,
                         auth: Option<zero_core::SessionAuth>| async move {
                            run_trojan_udp_relay(
                                engine_ref,
                                TcpRelayStream::new(stream.into_inner()),
                                session,
                                responder,
                                auth,
                                tag_ref,
                            )
                            .await
                        },
                    )
                    .await;
                if let Err(e) = result {
                    if !matches!(&e, EngineError::Io(io) if matches!(io.kind(),
                        io::ErrorKind::UnexpectedEof | io::ErrorKind::ConnectionReset | io::ErrorKind::BrokenPipe))
                    {
                        warn!(?source_addr, %e, "trojan failed");
                    }
                }
            }
        },
    })
    .await
}
