use std::future::Future;
use std::pin::Pin;

use zero_engine::EngineError;

use super::{PreparedInboundListenerOperation, TcpInboundListenerOperation};
use crate::protocol_registry::BoundInbound;
use crate::runtime::route_runtime::InboundListenerRuntime;

pub(crate) struct TcpAndDatagramInboundListenerOperation<R, D, U> {
    pub(crate) protocol_name: &'static str,
    pub(crate) error_protocol_name: &'static str,
    pub(crate) listen_address: String,
    pub(crate) listen_port: u16,
    pub(crate) tcp_request: R,
    pub(crate) tcp_dispatch: D,
    pub(crate) udp_relay: U,
}

impl<R, D, Fut, U> PreparedInboundListenerOperation
    for TcpAndDatagramInboundListenerOperation<R, D, U>
where
    R: Clone + Send + Sync + 'static,
    D: Fn(R, zero_platform_tokio::TokioSocket, super::InboundConnectionContext) -> Fut
        + Clone
        + Send
        + Sync
        + 'static,
    Fut: Future<Output = Result<(), EngineError>> + Send + 'static,
    U: zero_core::InboundDatagramUdpRelay<std::sync::Arc<tokio::net::UdpSocket>> + Send + 'static,
{
    fn execute(
        self: Box<Self>,
        runtime: InboundListenerRuntime,
        bound: BoundInbound,
        shutdown: tokio::sync::watch::Receiver<bool>,
    ) -> Pin<Box<dyn Future<Output = Result<(), EngineError>> + Send + 'static>> {
        Box::pin(async move {
            let TcpAndDatagramInboundListenerOperation {
                protocol_name,
                error_protocol_name,
                listen_address,
                listen_port,
                tcp_request,
                tcp_dispatch,
                udp_relay,
            } = *self;
            let udp_socket = match tokio::net::UdpSocket::bind(format!(
                "{listen_address}:{listen_port}"
            ))
            .await
            {
                Ok(socket) => Some(std::sync::Arc::new(socket)),
                Err(error) => {
                    tracing::warn!(%error, protocol = protocol_name, "failed to bind inbound UDP socket; UDP disabled");
                    None
                }
            };
            let udp_task = udp_socket.as_ref().map(|socket| {
                let udp_runtime = runtime.udp_runtime();
                let inbound_tag = runtime.inbound_tag().to_owned();
                let socket = socket.clone();
                tokio::spawn(async move {
                    crate::runtime::datagram_udp::run_protocol_datagram_udp_relay(
                        udp_runtime,
                        socket,
                        udp_relay,
                        &inbound_tag,
                        false,
                    )
                    .await
                })
            });

            let result = Box::new(TcpInboundListenerOperation {
                protocol_name,
                error_protocol_name,
                request: tcp_request,
                dispatch: tcp_dispatch,
            })
            .execute(runtime, bound, shutdown)
            .await;

            if let Some(task) = udp_task {
                task.abort();
                let _ = task.await;
            }
            result
        })
    }
}
