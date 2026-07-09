use async_trait::async_trait;
use socks5::Socks5InboundTcpAcceptor;
use tokio::sync::watch;
use zero_config::InboundConfig;
use zero_engine::EngineError;
use zero_traits::AsyncSocket;

use zero_core::Session;

use crate::logging::log_listener_connection_error;
use crate::runtime::inbound_protocol::{serve_inbound, InboundProtocol};
use crate::runtime::listener_loop::{run_tcp_listener_loop, TcpListenerLoopRequest};
use crate::runtime::Proxy;
use crate::transport::{MeteredStream, TcpRelayStream};

use super::request::Socks5InboundListenerRequest;

#[path = "udp_associate.rs"]
pub(crate) mod udp_associate;

pub(crate) async fn handle_socks5_connection(
    proxy: &Proxy,
    inbound_tag: &str,
    source_addr: Option<std::net::SocketAddr>,
    mut metered: MeteredStream<TcpRelayStream>,
    acceptor: &Socks5InboundTcpAcceptor,
    protocol_name: &'static str,
) {
    match acceptor.accept_command(&mut metered).await {
        Ok(request) => {
            let _ = request
                .dispatch_with_handlers(
                    metered,
                    |session: Session, stream| {
                        serve_inbound(
                            proxy,
                            session,
                            stream.into_inner(),
                            acceptor,
                            inbound_tag,
                            source_addr,
                        )
                    },
                    |request, stream| {
                        udp_associate::run_socks5_udp_associate(proxy, stream, inbound_tag, request)
                    },
                )
                .await;
        }
        Err(err) => {
            let engine_err = EngineError::from(err);
            log_listener_connection_error(protocol_name, inbound_tag, &source_addr, &engine_err);
        }
    }
}

#[async_trait]
impl InboundProtocol for Socks5InboundTcpAcceptor {
    type ClientStream = TcpRelayStream;

    async fn accept(
        &self,
        stream: TcpRelayStream,
    ) -> Result<(Session, Self::ClientStream), EngineError> {
        let mut metered = MeteredStream::new(stream);
        let session = Socks5InboundTcpAcceptor::accept_request(self, &mut metered).await?;
        Ok((session, metered.into_inner()))
    }

    async fn send_ok(&self, client: &mut TcpRelayStream) -> Result<(), EngineError> {
        Socks5InboundTcpAcceptor::send_success(self, client)
            .await
            .map_err(EngineError::from)
    }

    async fn send_blocked(&self, client: &mut TcpRelayStream) -> Result<(), EngineError> {
        let _ = Socks5InboundTcpAcceptor::send_blocked(self, client).await;
        let _ = client.shutdown().await;
        Ok(())
    }

    async fn send_upstream_failure(&self, client: &mut TcpRelayStream) -> Result<(), EngineError> {
        let _ = Socks5InboundTcpAcceptor::send_upstream_failure(self, client).await;
        let _ = client.shutdown().await;
        Ok(())
    }
}

pub(crate) async fn run_socks5_listener_with_bound(
    proxy: &Proxy,
    inbound: InboundConfig,
    request: Socks5InboundListenerRequest,
    listener: zero_platform_tokio::TokioListener,
    shutdown: watch::Receiver<bool>,
) -> Result<(), EngineError> {
    let Socks5InboundListenerRequest { acceptor } = request;
    run_tcp_listener_loop(TcpListenerLoopRequest {
        proxy,
        inbound_tag: inbound.tag,
        protocol_name: "socks5",
        listener,
        shutdown,
        handler: move |engine: Proxy,
                       tag: String,
                       stream: zero_platform_tokio::TokioSocket,
                       source_addr: Option<std::net::SocketAddr>| {
            let acceptor = acceptor.clone();
            async move {
                handle_socks5_connection(
                    &engine,
                    &tag,
                    source_addr,
                    MeteredStream::new(TcpRelayStream::from(stream)),
                    &acceptor,
                    "socks5",
                )
                .await;
            }
        },
    })
    .await
}
