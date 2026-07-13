use async_trait::async_trait;
use http::HttpConnectInbound;
use tokio::io::AsyncWriteExt;
use zero_engine::EngineError;

use crate::runtime::inbound_operation::{InboundConnectionContext, TcpInboundListenerOperation};
use crate::runtime::tcp_ingress::InboundProtocol;
use crate::transport::{MeteredStream, TcpRelayStream};

#[derive(Clone, Copy)]
pub(crate) struct HttpConnectInboundHandler {
    http_inbound: HttpConnectInbound,
}

impl Default for HttpConnectInboundHandler {
    fn default() -> Self {
        Self {
            http_inbound: HttpConnectInbound,
        }
    }
}

impl HttpConnectInboundHandler {
    pub(crate) fn http_inbound(&self) -> HttpConnectInbound {
        self.http_inbound
    }
}

#[async_trait]
impl InboundProtocol for HttpConnectInboundHandler {
    type ClientStream = TcpRelayStream;

    async fn send_ok(&self, client: &mut TcpRelayStream) -> Result<(), EngineError> {
        self.http_inbound
            .send_success_response(client)
            .await
            .map_err(EngineError::from)
    }

    async fn send_blocked(&self, client: &mut TcpRelayStream) -> Result<(), EngineError> {
        let _ = self.http_inbound.send_blocked_response(client).await;
        let _ = AsyncWriteExt::shutdown(client).await;
        Ok(())
    }

    async fn send_upstream_failure(&self, client: &mut TcpRelayStream) -> Result<(), EngineError> {
        let _ = self
            .http_inbound
            .send_upstream_failure_response(client)
            .await;
        let _ = AsyncWriteExt::shutdown(client).await;
        Ok(())
    }
}

impl crate::adapters::http::HttpConnectAdapter {
    pub(super) fn prepare_inbound_listener_impl(
        &self,
        inbound: zero_config::InboundConfig,
    ) -> Result<
        Box<dyn crate::runtime::inbound_operation::PreparedInboundListenerOperation>,
        EngineError,
    > {
        Ok(Box::new(TcpInboundListenerOperation {
            inbound_tag: inbound.tag,
            protocol_name: "http",
            error_protocol_name: "http",
            request: HttpConnectInboundHandler::default(),
            dispatch: |handler: HttpConnectInboundHandler,
                       socket,
                       context: InboundConnectionContext| async move {
                let mut metered = MeteredStream::new(TcpRelayStream::from(socket));
                match handler.http_inbound.accept_request(&mut metered).await {
                    Ok(session) => {
                        if let Some((status, location)) = context.select_http_redirect(&session) {
                            handler
                                .http_inbound
                                .send_redirect_response(&mut metered, status, &location)
                                .await
                                .map_err(EngineError::from)
                        } else {
                            context.serve(session, metered.into_inner(), handler).await
                        }
                    }
                    Err(error) => {
                        if handler
                            .http_inbound
                            .send_accept_error_response(&mut metered, &error)
                            .await
                            .unwrap_or(false)
                        {
                            Ok(())
                        } else {
                            Err(EngineError::from(error))
                        }
                    }
                }
            },
        }))
    }
}
