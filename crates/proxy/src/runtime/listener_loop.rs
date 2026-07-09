use std::future::Future;
use std::path::Path;

use tokio::net::TcpStream;
use tokio::sync::watch;
use tokio::task::JoinSet;
use tracing::{error, info};
use zero_config::{InboundConfig, InboundProtocolConfig};
use zero_engine::EngineError;
use zero_stack::SystemTcpStack;
use zero_traits::TcpStack;

use crate::protocol_registry::BoundInbound;
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

pub(crate) fn spawn_logged_tcp_inbound_listener<R, B, N, D, Fut>(
    proxy: &Proxy,
    inbound: InboundConfig,
    bound: BoundInbound,
    shutdown_rx: watch::Receiver<bool>,
    listeners: &mut JoinSet<Result<(), EngineError>>,
    build_request: B,
    protocol_name: N,
    error_protocol_name: &'static str,
    dispatch: D,
) where
    R: Clone + Send + Sync + 'static,
    B: FnOnce(&InboundProtocolConfig, Option<&Path>) -> Result<R, EngineError> + Send + 'static,
    N: FnOnce(&R) -> &'static str + Send + 'static,
    D: Fn(Proxy, R, String, zero_platform_tokio::TokioSocket, Option<std::net::SocketAddr>) -> Fut
        + Clone
        + Send
        + Sync
        + 'static,
    Fut: Future<Output = Result<(), EngineError>> + Send + 'static,
{
    let proxy = proxy.clone();
    listeners.spawn(async move {
        let request = build_request(&inbound.protocol, proxy.config.source_dir())?;
        let protocol_name = protocol_name(&request);
        run_logged_tcp_socket_listener_loop(LoggedTcpSocketListenerRequest {
            proxy: &proxy,
            inbound_tag: inbound.tag,
            protocol_name,
            error_protocol_name,
            request,
            listener: bound.into_tcp(),
            shutdown: shutdown_rx,
            dispatch,
        })
        .await
    });
}

pub(crate) struct SystemTcpStackLoopRequest<'a, H> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) inbound_tag: String,
    pub(crate) stack: SystemTcpStack,
    pub(crate) shutdown: watch::Receiver<bool>,
    pub(crate) handler: H,
}

pub(crate) async fn run_system_tcp_stack_loop<H, Fut>(request: SystemTcpStackLoopRequest<'_, H>)
where
    H: Fn(Proxy, String, TcpStream, zero_traits::SocketAddress, zero_traits::SocketAddress) -> Fut
        + Clone
        + Send
        + Sync
        + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    let SystemTcpStackLoopRequest {
        proxy,
        inbound_tag,
        stack,
        mut shutdown,
        handler,
    } = request;
    let mut connections = JoinSet::new();

    loop {
        tokio::select! {
            biased;

            changed = shutdown.changed() => {
                match changed {
                    Ok(()) if *shutdown.borrow() => {
                        info!(inbound_tag = %inbound_tag, "system inbound shutdown");
                        break;
                    }
                    Ok(()) => {}
                    Err(_) => break,
                }
            }

            accepted = stack.accept() => {
                match accepted {
                    Some((stream, source, destination)) => {
                        let engine = proxy.clone();
                        let tag = inbound_tag.clone();
                        let handler = handler.clone();
                        connections.spawn(handler(engine, tag, stream, source, destination));
                    }
                    None => break,
                }
            }

            result = connections.join_next(), if !connections.is_empty() => {
                if let Some(Err(error)) = result {
                    if !error.is_cancelled() {
                        error!(error = %error, "system connection task panicked");
                    }
                }
            }
        }
    }

    connections.abort_all();
    while let Some(result) = connections.join_next().await {
        if let Err(error) = result {
            if !error.is_cancelled() {
                error!(error = %error, "system connection task panicked during shutdown");
            }
        }
    }
}

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
    H: Fn(Proxy, String, quinn::Connection) -> Fut + Clone + Send + Sync + 'static,
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
                        let engine = proxy.clone();
                        let tag = inbound_tag.clone();
                        let handler = handler.clone();
                        connections.spawn(handler(engine, tag, conn));
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

#[cfg(feature = "transport_quic")]
pub(crate) struct QuicStreamListenerLoopRequest<'a, H> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) inbound_tag: String,
    pub(crate) protocol_name: &'static str,
    pub(crate) listener: crate::transport::QuicInbound,
    pub(crate) shutdown: watch::Receiver<bool>,
    pub(crate) handler: H,
}

#[cfg(feature = "transport_quic")]
pub(crate) async fn run_quic_stream_listener_loop<H, Fut>(
    request: QuicStreamListenerLoopRequest<'_, H>,
) -> Result<(), EngineError>
where
    H: Fn(Proxy, String, crate::transport::QuicStream) -> Fut + Clone + Send + Sync + 'static,
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
                        let engine = proxy.clone();
                        let tag = inbound_tag.clone();
                        let handler = handler.clone();
                        connections.spawn(handler(engine, tag, stream));
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

#[cfg(feature = "transport_quic")]
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

#[cfg(feature = "transport_quic")]
pub(crate) async fn run_logged_quic_stream_listener_loop<R, D, Fut>(
    request: LoggedQuicStreamListenerRequest<'_, R, D>,
) -> Result<(), EngineError>
where
    R: Clone + Send + Sync + 'static,
    D: Fn(Proxy, R, String, crate::transport::QuicStream) -> Fut + Clone + Send + Sync + 'static,
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
        handler: move |engine: Proxy,
                       inbound_tag: String,
                       quic_stream: crate::transport::QuicStream| {
            let request = request.clone();
            let dispatch = dispatch.clone();
            async move {
                let log_tag = inbound_tag.clone();
                let result = dispatch(engine, request, inbound_tag, quic_stream).await;
                if let Err(error) = &result {
                    crate::logging::log_listener_connection_error(
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

#[cfg(feature = "transport_quic")]
pub(crate) fn spawn_logged_bound_inbound_listener<R, B, N, DTcp, FTcp, DQuic, FQuic>(
    proxy: &Proxy,
    inbound: InboundConfig,
    bound: BoundInbound,
    shutdown_rx: watch::Receiver<bool>,
    listeners: &mut JoinSet<Result<(), EngineError>>,
    build_request: B,
    protocol_name: N,
    error_protocol_name: &'static str,
    dispatch_tcp: DTcp,
    dispatch_quic: DQuic,
) where
    R: Clone + Send + Sync + 'static,
    B: FnOnce(&InboundProtocolConfig, Option<&Path>) -> Result<R, EngineError> + Send + 'static,
    N: FnOnce(&R) -> &'static str + Send + 'static,
    DTcp: Fn(Proxy, R, String, zero_platform_tokio::TokioSocket, Option<std::net::SocketAddr>) -> FTcp
        + Clone
        + Send
        + Sync
        + 'static,
    FTcp: Future<Output = Result<(), EngineError>> + Send + 'static,
    DQuic:
        Fn(Proxy, R, String, crate::transport::QuicStream) -> FQuic + Clone + Send + Sync + 'static,
    FQuic: Future<Output = Result<(), EngineError>> + Send + 'static,
{
    let proxy = proxy.clone();
    listeners.spawn(async move {
        let request = build_request(&inbound.protocol, proxy.config.source_dir())?;
        let protocol_name = protocol_name(&request);
        let inbound_tag = inbound.tag;

        match bound {
            BoundInbound::Tcp(listener) => {
                run_logged_tcp_socket_listener_loop(LoggedTcpSocketListenerRequest {
                    proxy: &proxy,
                    inbound_tag,
                    protocol_name,
                    error_protocol_name,
                    request,
                    listener,
                    shutdown: shutdown_rx,
                    dispatch: dispatch_tcp,
                })
                .await
            }
            BoundInbound::Quic(listener) => {
                run_logged_quic_stream_listener_loop(LoggedQuicStreamListenerRequest {
                    proxy: &proxy,
                    inbound_tag,
                    protocol_name,
                    error_protocol_name,
                    request,
                    listener,
                    shutdown: shutdown_rx,
                    dispatch: dispatch_quic,
                })
                .await
            }
        }
    });
}
