use std::future::Future;

use tokio::sync::watch;
use zero_engine::EngineError;

use crate::runtime::route_runtime::{InboundRouteRuntime, InboundRouteRuntimeFactory};

use super::stream::{run_quic_stream_listener_loop, QuicStreamListenerLoopRequest};

pub(crate) struct LoggedQuicStreamListenerRequest<R, D> {
    pub(crate) runtime_factory: InboundRouteRuntimeFactory,
    pub(crate) protocol_name: &'static str,
    pub(crate) error_protocol_name: &'static str,
    pub(crate) request: R,
    pub(crate) listener: crate::transport::QuicInbound,
    pub(crate) shutdown: watch::Receiver<bool>,
    pub(crate) dispatch: D,
}

pub(crate) async fn run_logged_quic_stream_listener_loop<R, D, Fut>(
    request: LoggedQuicStreamListenerRequest<R, D>,
) -> Result<(), EngineError>
where
    R: Clone + Send + Sync + 'static,
    D: Fn(InboundRouteRuntime, R, crate::transport::QuicStream) -> Fut
        + Clone
        + Send
        + Sync
        + 'static,
    Fut: Future<Output = Result<(), EngineError>> + Send + 'static,
{
    let LoggedQuicStreamListenerRequest {
        runtime_factory,
        protocol_name,
        error_protocol_name,
        request,
        listener,
        shutdown,
        dispatch,
    } = request;

    run_quic_stream_listener_loop(QuicStreamListenerLoopRequest {
        runtime_factory,
        protocol_name,
        listener,
        shutdown,
        handler: move |runtime: InboundRouteRuntime, quic_stream: crate::transport::QuicStream| {
            let request = request.clone();
            let dispatch = dispatch.clone();
            async move {
                let log_tag = runtime.inbound_tag().to_owned();
                let result = dispatch(runtime, request, quic_stream).await;
                if let Err(error) = &result {
                    crate::logging::log_listener_connection_error(
                        crate::logging::INBOUND_ACCEPT_ROUTE_STAGE,
                        error_protocol_name,
                        log_tag.as_str(),
                        &"quic",
                        error,
                    );
                }
            }
        },
    })
    .await
}
