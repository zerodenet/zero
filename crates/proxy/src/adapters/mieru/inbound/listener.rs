//! Mieru inbound encrypted handshake and AEAD-framed relay.

mod udp;

use async_trait::async_trait;
use mieru::{MieruInbound, MieruInboundAcceptedSessionDispatcher, MieruInboundProfile};
use tokio::sync::watch;
use zero_config::InboundConfig;
use zero_core::Session;
use zero_engine::EngineError;

use crate::logging::log_listener_connection_error;
use crate::runtime::inbound_protocol::{serve_inbound, InboundProtocol};
use crate::runtime::listener_loop::{run_tcp_listener_loop, TcpListenerLoopRequest};
use crate::runtime::Proxy;
use crate::transport::{MeteredStream, TcpRelayStream};

type MieruClientStream = mieru::MieruInboundStream<MeteredStream<TcpRelayStream>>;

#[derive(Debug)]
pub(crate) struct MieruInboundRequest {
    pub(crate) inbound: InboundConfig,
    pub(crate) profile: MieruInboundProfile,
}

// Handler.

#[derive(Clone)]
pub(crate) struct MieruInboundHandler {
    mieru_inbound: MieruInbound,
    profile: MieruInboundProfile,
}

struct MieruAcceptedSessionBridge<'a> {
    proxy: &'a Proxy,
    handler: &'a MieruInboundHandler,
    inbound_tag: &'a str,
    source_addr: Option<std::net::SocketAddr>,
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
    ) -> Result<mieru::MieruInboundAcceptedSession<MieruClientStream>, EngineError> {
        let metered = MeteredStream::new(stream);
        self.profile
            .accept_client(&self.mieru_inbound, metered)
            .await
            .map_err(EngineError::from)
    }
}

impl MieruInboundAcceptedSessionDispatcher<MieruClientStream> for MieruAcceptedSessionBridge<'_> {
    type Error = EngineError;

    async fn dispatch_tcp_session(
        &mut self,
        session: Session,
        stream: MieruClientStream,
    ) -> Result<(), Self::Error> {
        serve_inbound(
            self.proxy,
            session,
            stream,
            self.handler,
            self.inbound_tag,
            self.source_addr,
        )
        .await
    }

    async fn dispatch_udp_session(
        &mut self,
        session: Session,
        stream: MieruClientStream,
        responder: mieru::udp::MieruInboundUdpResponder,
        auth: Option<zero_core::SessionAuth>,
    ) -> Result<(), Self::Error> {
        self.proxy
            .run_mieru_udp_relay(stream, &session, responder, auth, self.inbound_tag)
            .await
    }
}

// Listener.

pub(crate) async fn run_mieru_listener_with_bound(
    proxy: &Proxy,
    request: MieruInboundRequest,
    listener: zero_platform_tokio::TokioListener,
    shutdown: watch::Receiver<bool>,
) -> Result<(), EngineError> {
    let MieruInboundRequest { inbound, profile } = request;

    let handler = MieruInboundHandler {
        mieru_inbound: MieruInbound,
        profile,
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
                        let mut bridge = MieruAcceptedSessionBridge {
                            proxy: &engine,
                            handler: &handler,
                            inbound_tag: &tag,
                            source_addr,
                        };
                        let result = client.dispatch_with(&mut bridge).await;
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
