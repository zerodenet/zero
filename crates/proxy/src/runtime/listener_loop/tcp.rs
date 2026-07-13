use std::future::Future;

use tokio::sync::watch;
use tokio::task::JoinSet;
use tracing::{error, info};
use zero_engine::EngineError;

use crate::runtime::Proxy;

pub(crate) struct TcpListenerLoopRequest<'a, H> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) inbound_tag: String,
    pub(crate) protocol_name: &'static str,
    pub(crate) listener: zero_platform_tokio::TokioListener,
    pub(crate) shutdown: watch::Receiver<bool>,
    pub(crate) handler: H,
}

pub(crate) async fn run_tcp_listener_loop<H, Fut>(
    request: TcpListenerLoopRequest<'_, H>,
) -> Result<(), EngineError>
where
    H: Fn(Proxy, String, zero_platform_tokio::TokioSocket, Option<std::net::SocketAddr>) -> Fut
        + Clone
        + Send
        + Sync
        + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    let TcpListenerLoopRequest {
        proxy,
        inbound_tag,
        protocol_name,
        listener,
        mut shutdown,
        handler,
    } = request;
    let local_addr = listener.local_addr()?;
    let mut connections = JoinSet::new();

    info!(
        inbound_tag = %inbound_tag,
        protocol = protocol_name,
        listen = %local_addr,
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
                    Ok((stream, remote_addr)) => {
                        let engine = proxy.clone();
                        let tag = inbound_tag.clone();
                        let source_addr = zero_platform_tokio::remote_ip_to_socket_addr(remote_addr);
                        let handler = handler.clone();
                        connections.spawn(handler(engine, tag, stream, source_addr));
                    }
                    Err(error) => {
                        error!(error = %error, protocol = protocol_name, "inbound accept error");
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
        listen = %local_addr,
        "inbound listener stopped"
    );
    Ok(())
}

#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
pub(crate) struct LoggedTcpSocketListenerRequest<'a, R, D> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) inbound_tag: String,
    pub(crate) protocol_name: &'static str,
    pub(crate) error_protocol_name: &'static str,
    pub(crate) request: R,
    pub(crate) listener: zero_platform_tokio::TokioListener,
    pub(crate) shutdown: watch::Receiver<bool>,
    pub(crate) dispatch: D,
}

#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
pub(crate) async fn run_logged_tcp_socket_listener_loop<R, D, Fut>(
    request: LoggedTcpSocketListenerRequest<'_, R, D>,
) -> Result<(), EngineError>
where
    R: Clone + Send + Sync + 'static,
    D: Fn(Proxy, R, String, zero_platform_tokio::TokioSocket, Option<std::net::SocketAddr>) -> Fut
        + Clone
        + Send
        + Sync
        + 'static,
    Fut: Future<Output = Result<(), EngineError>> + Send + 'static,
{
    let LoggedTcpSocketListenerRequest {
        proxy,
        inbound_tag,
        protocol_name,
        error_protocol_name,
        request,
        listener,
        shutdown,
        dispatch,
    } = request;

    run_tcp_listener_loop(TcpListenerLoopRequest {
        proxy,
        inbound_tag,
        protocol_name,
        listener,
        shutdown,
        handler: move |engine: Proxy,
                       inbound_tag: String,
                       stream: zero_platform_tokio::TokioSocket,
                       source_addr: Option<std::net::SocketAddr>| {
            let request = request.clone();
            let dispatch = dispatch.clone();
            async move {
                let log_tag = inbound_tag.clone();
                let result = dispatch(engine, request, inbound_tag, stream, source_addr).await;
                if let Err(ref error) = result {
                    crate::logging::log_listener_connection_error(
                        crate::logging::INBOUND_ACCEPT_ROUTE_STAGE,
                        error_protocol_name,
                        log_tag.as_str(),
                        &source_addr,
                        error,
                    );
                }
            }
        },
    })
    .await
}
