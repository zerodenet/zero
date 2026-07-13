use async_trait::async_trait;
use http_connect::HttpConnectInbound;
use tokio::io::AsyncWriteExt;
use tokio::sync::watch;
use zero_engine::EngineError;

use crate::logging::log_listener_connection_error;
use crate::runtime::http_redirect::select_redirect_target;
use crate::runtime::listener_loop::{run_tcp_listener_loop, TcpListenerLoopRequest};
use crate::runtime::tcp_ingress::{serve_inbound, InboundProtocol};
use crate::runtime::Proxy;
use crate::transport::{MeteredStream, TcpRelayStream};

#[derive(Clone, Copy)]
pub(crate) struct HttpConnectInboundHandler {
    http_connect_inbound: HttpConnectInbound,
}

impl Default for HttpConnectInboundHandler {
    fn default() -> Self {
        Self {
            http_connect_inbound: HttpConnectInbound,
        }
    }
}

impl HttpConnectInboundHandler {
    pub(crate) fn http_connect_inbound(&self) -> HttpConnectInbound {
        self.http_connect_inbound
    }
}

#[async_trait]
impl InboundProtocol for HttpConnectInboundHandler {
    type ClientStream = TcpRelayStream;

    async fn send_ok(&self, client: &mut TcpRelayStream) -> Result<(), EngineError> {
        self.http_connect_inbound
            .send_success_response(client)
            .await
            .map_err(EngineError::from)
    }

    async fn send_blocked(&self, client: &mut TcpRelayStream) -> Result<(), EngineError> {
        let _ = self
            .http_connect_inbound
            .send_blocked_response(client)
            .await;
        let _ = AsyncWriteExt::shutdown(client).await;
        Ok(())
    }

    async fn send_upstream_failure(&self, client: &mut TcpRelayStream) -> Result<(), EngineError> {
        let _ = self
            .http_connect_inbound
            .send_upstream_failure_response(client)
            .await;
        let _ = AsyncWriteExt::shutdown(client).await;
        Ok(())
    }
}

pub(crate) async fn run_http_connect_listener_with_bound(
    proxy: &Proxy,
    inbound: zero_config::InboundConfig,
    listener: zero_platform_tokio::TokioListener,
    shutdown: watch::Receiver<bool>,
) -> Result<(), EngineError> {
    let handler = HttpConnectInboundHandler::default();

    run_tcp_listener_loop(TcpListenerLoopRequest {
        proxy,
        inbound_tag: inbound.tag,
        protocol_name: "http_connect",
        listener,
        shutdown,
        handler: move |engine: Proxy,
                       tag: String,
                       stream: zero_platform_tokio::TokioSocket,
                       source_addr: Option<std::net::SocketAddr>| {
            let handler = handler;
            async move {
                let mut metered = MeteredStream::new(TcpRelayStream::from(stream));
                match handler
                    .http_connect_inbound
                    .accept_request(&mut metered)
                    .await
                {
                    Ok(session) => {
                        if let Some((status, location)) =
                            select_redirect_target(&engine.config.route.url_rewrite, &session)
                        {
                            let _ = handler
                                .http_connect_inbound
                                .send_redirect_response(&mut metered, status, &location)
                                .await;
                        } else {
                            let _ = serve_inbound(
                                &engine,
                                session,
                                metered.into_inner(),
                                &handler,
                                &tag,
                                source_addr,
                            )
                            .await;
                        }
                    }
                    Err(error) => {
                        if handler
                            .http_connect_inbound
                            .send_accept_error_response(&mut metered, &error)
                            .await
                            .unwrap_or(false)
                        {
                            return;
                        }
                        let engine_error = EngineError::from(error);
                        log_listener_connection_error(
                            crate::logging::INBOUND_ACCEPT_ROUTE_STAGE,
                            "http_connect",
                            &tag,
                            &source_addr,
                            &engine_error,
                        );
                    }
                }
            }
        },
    })
    .await
}
