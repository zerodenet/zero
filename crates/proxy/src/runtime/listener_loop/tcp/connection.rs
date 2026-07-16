use std::future::Future;

use tokio::sync::watch;
use tokio::task::JoinSet;
use tracing::{error, info};
use zero_engine::EngineError;

use crate::runtime::route_runtime::{InboundRouteRuntime, InboundRouteRuntimeFactory};

pub(crate) struct TcpListenerLoopRequest<H> {
    pub(crate) runtime_factory: InboundRouteRuntimeFactory,
    pub(crate) protocol_name: &'static str,
    pub(crate) listener: zero_platform_tokio::TokioListener,
    pub(crate) shutdown: watch::Receiver<bool>,
    pub(crate) handler: H,
}

pub(crate) async fn run_tcp_listener_loop<H, Fut>(
    request: TcpListenerLoopRequest<H>,
) -> Result<(), EngineError>
where
    H: Fn(InboundRouteRuntime, zero_platform_tokio::TokioSocket) -> Fut
        + Clone
        + Send
        + Sync
        + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    let TcpListenerLoopRequest {
        runtime_factory,
        protocol_name,
        listener,
        mut shutdown,
        handler,
    } = request;
    let local_addr = listener.local_addr()?;
    let mut connections = JoinSet::new();

    info!(
        inbound_tag = %runtime_factory.inbound_tag(),
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
                        let runtime = runtime_factory.for_connection(
                            zero_platform_tokio::remote_ip_to_socket_addr(remote_addr),
                        );
                        let handler = handler.clone();
                        connections.spawn(handler(runtime, stream));
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
        inbound_tag = %runtime_factory.inbound_tag(),
        protocol = protocol_name,
        listen = %local_addr,
        "inbound listener stopped"
    );
    Ok(())
}
