use std::future::Future;
use tokio::sync::watch;
use tokio::task::JoinSet;
use tracing::{error, info};
use zero_engine::EngineError;

use crate::runtime::route_runtime::InboundRouteRuntime;
use crate::runtime::Proxy;

#[cfg(feature = "hysteria2")]
pub(crate) struct QuicListenerLoopRequest<'a, H> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) inbound_tag: String,
    pub(crate) protocol_name: &'static str,
    pub(crate) listener: crate::transport::QuicInbound,
    pub(crate) shutdown: watch::Receiver<bool>,
    pub(crate) handler: H,
}

#[cfg(feature = "hysteria2")]
pub(crate) async fn run_quic_listener_loop<H, Fut>(
    request: QuicListenerLoopRequest<'_, H>,
) -> Result<(), EngineError>
where
    H: Fn(InboundRouteRuntime, quinn::Connection) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    let QuicListenerLoopRequest {
        proxy,
        inbound_tag,
        protocol_name,
        listener,
        mut shutdown,
        handler,
    } = request;
    let mut connections = JoinSet::new();

    info!(
        inbound_tag = %inbound_tag,
        protocol = protocol_name,
        "inbound listener ready"
    );

    loop {
        tokio::select! {
            changed = shutdown.changed() => {
                match changed {
                    Ok(()) if *shutdown.borrow() => break,
                    Ok(()) => {}
                    Err(_) => break,
                }
            }
            accept_result = listener.accept_connection() => {
                match accept_result {
                    Ok(conn) => {
                        let runtime = InboundRouteRuntime::new(proxy.clone(), inbound_tag.clone(), None);
                        let handler = handler.clone();
                        connections.spawn(handler(runtime, conn));
                    }
                    Err(error) => {
                        error!(error = %error, protocol = protocol_name, "inbound accept error");
                        break;
                    }
                }
            }
            result = connections.join_next(), if !connections.is_empty() => {
                if let Some(Err(error)) = result {
                    if !error.is_cancelled() {
                        error!(error = %error, protocol = protocol_name, "inbound connection task panicked");
                    }
                }
            }
        }
    }

    connections.abort_all();
    while let Some(result) = connections.join_next().await {
        if let Err(error) = result {
            if !error.is_cancelled() {
                error!(error = %error, protocol = protocol_name, "inbound connection task panicked during shutdown");
            }
        }
    }

    info!(
        inbound_tag = %inbound_tag,
        protocol = protocol_name,
        "inbound listener stopped"
    );
    Ok(())
}

#[cfg(feature = "vless")]
pub(crate) struct QuicStreamListenerLoopRequest<'a, H> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) inbound_tag: String,
    pub(crate) protocol_name: &'static str,
    pub(crate) listener: crate::transport::QuicInbound,
    pub(crate) shutdown: watch::Receiver<bool>,
    pub(crate) handler: H,
}

#[cfg(feature = "vless")]
pub(crate) async fn run_quic_stream_listener_loop<H, Fut>(
    request: QuicStreamListenerLoopRequest<'_, H>,
) -> Result<(), EngineError>
where
    H: Fn(InboundRouteRuntime, crate::transport::QuicStream) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    let QuicStreamListenerLoopRequest {
        proxy,
        inbound_tag,
        protocol_name,
        listener,
        mut shutdown,
        handler,
    } = request;
    let mut connections = JoinSet::new();

    info!(
        inbound_tag = %inbound_tag,
        protocol = protocol_name,
        transport = "quic",
        "inbound listener ready"
    );

    loop {
        tokio::select! {
            changed = shutdown.changed() => {
                match changed {
                    Ok(()) if *shutdown.borrow() => break,
                    Ok(()) => {}
                    Err(_) => break,
                }
            }
            accept_result = listener.accept() => {
                match accept_result {
                    Ok(stream) => {
                        let runtime = InboundRouteRuntime::new(proxy.clone(), inbound_tag.clone(), None);
                        let handler = handler.clone();
                        connections.spawn(handler(runtime, stream));
                    }
                    Err(error) => {
                        error!(error = %error, protocol = protocol_name, "inbound accept error");
                        break;
                    }
                }
            }
            result = connections.join_next(), if !connections.is_empty() => {
                if let Some(Err(error)) = result {
                    if !error.is_cancelled() {
                        error!(error = %error, protocol = protocol_name, "inbound connection task panicked");
                    }
                }
            }
        }
    }

    connections.abort_all();
    while let Some(result) = connections.join_next().await {
        if let Err(error) = result {
            if !error.is_cancelled() {
                error!(error = %error, protocol = protocol_name, "inbound connection task panicked during shutdown");
            }
        }
    }

    info!(
        inbound_tag = %inbound_tag,
        protocol = protocol_name,
        transport = "quic",
        "inbound listener stopped"
    );
    Ok(())
}

#[cfg(feature = "vless")]
pub(crate) struct LoggedQuicStreamListenerRequest<'a, R, D> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) inbound_tag: String,
    pub(crate) protocol_name: &'static str,
    pub(crate) error_protocol_name: &'static str,
    pub(crate) request: R,
    pub(crate) listener: crate::transport::QuicInbound,
    pub(crate) shutdown: watch::Receiver<bool>,
    pub(crate) dispatch: D,
}

#[cfg(feature = "vless")]
pub(crate) async fn run_logged_quic_stream_listener_loop<R, D, Fut>(
    request: LoggedQuicStreamListenerRequest<'_, R, D>,
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
        proxy,
        inbound_tag,
        protocol_name,
        error_protocol_name,
        request,
        listener,
        shutdown,
        dispatch,
    } = request;

    run_quic_stream_listener_loop(QuicStreamListenerLoopRequest {
        proxy,
        inbound_tag,
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
