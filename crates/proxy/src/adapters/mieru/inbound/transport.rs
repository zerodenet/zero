//! Mieru inbound encrypted handshake and AEAD-framed relay.

#[path = "udp.rs"]
mod udp;

use async_trait::async_trait;
use tokio::sync::watch;
use zero_config::InboundConfig;
use zero_core::Session;
use zero_engine::EngineError;

use crate::logging::log_listener_connection_error;
use crate::runtime::inbound_protocol::{serve_inbound, InboundProtocol};
use crate::runtime::listener_loop::{run_tcp_listener_loop, TcpListenerLoopRequest};
use crate::runtime::Proxy;
use crate::transport::{MeteredStream, TcpRelayStream};

use super::request::MieruInboundListenerRequest;

type MieruClientStream = mieru::inbound::MieruInboundStream<MeteredStream<TcpRelayStream>>;

// Handler.

#[derive(Clone)]
pub(crate) struct MieruInboundHandler {
    mieru_inbound: mieru::inbound::MieruInbound,
    profile: mieru::inbound::MieruInboundProfile,
}

#[async_trait]
impl InboundProtocol for MieruInboundHandler {
    type ClientStream = MieruClientStream;

    async fn accept(
        &self,
        stream: TcpRelayStream,
    ) -> Result<(Session, Self::ClientStream), EngineError> {
        let _ = stream;
        unreachable!("mieru accept is handled before serve_inbound dispatch")
    }

    async fn send_ok(&self, _client: &mut Self::ClientStream) -> Result<(), EngineError> {
        Ok(()) // Mieru handshake already confirms success
    }

    async fn send_blocked(&self, _client: &mut Self::ClientStream) -> Result<(), EngineError> {
        // Mieru protocol has no explicit blocked response;
        // the connection close serves as the signal.
        Ok(())
    }

    async fn send_upstream_failure(
        &self,
        _client: &mut Self::ClientStream,
    ) -> Result<(), EngineError> {
        self.send_blocked(_client).await
    }
}

impl MieruInboundHandler {
    async fn accept_client(
        &self,
        stream: TcpRelayStream,
    ) -> Result<mieru::inbound::MieruInboundAcceptedSession<MieruClientStream>, EngineError> {
        let metered = MeteredStream::new(stream);
        self.profile
            .accept_client(&self.mieru_inbound, metered)
            .await
            .map_err(EngineError::from)
    }
}

// Listener.

pub(crate) async fn run_mieru_listener_with_bound(
    proxy: &Proxy,
    inbound: InboundConfig,
    request: MieruInboundListenerRequest,
    listener: zero_platform_tokio::TokioListener,
    shutdown: watch::Receiver<bool>,
) -> Result<(), EngineError> {
    let handler = MieruInboundHandler {
        mieru_inbound: mieru::inbound::MieruInbound,
        profile: request.profile,
    };

    run_tcp_listener_loop(TcpListenerLoopRequest {
        proxy,
        inbound_tag: inbound.tag,
        protocol_name: "mieru",
        listener,
        shutdown,
        handler: move |engine: Proxy,
                       tag: String,
                       stream: zero_platform_tokio::TokioSocket,
                       source_addr: Option<std::net::SocketAddr>| {
            let handler = handler.clone();
            async move {
                match handler.accept_client(stream.into()).await {
                    Ok(client) => {
                        let engine_ref = &engine;
                        let handler_ref = &handler;
                        let inbound_tag = tag.as_str();
                        let result = client
                            .dispatch(
                                |session: Session, stream| async move {
                                    serve_inbound(
                                        engine_ref,
                                        session,
                                        stream,
                                        handler_ref,
                                        inbound_tag,
                                        source_addr,
                                    )
                                    .await
                                },
                                |session, stream, responder, auth| async move {
                                    udp::run_mieru_udp_relay(
                                        engine_ref,
                                        stream,
                                        &session,
                                        responder,
                                        auth,
                                        inbound_tag,
                                    )
                                    .await
                                },
                            )
                            .await;
                        if let Err(error) = result {
                            log_listener_connection_error("mieru", &tag, &source_addr, &error);
                        }
                    }
                    Err(error) => {
                        log_listener_connection_error("mieru", &tag, &source_addr, &error);
                    }
                }
            }
        },
    })
    .await
}
