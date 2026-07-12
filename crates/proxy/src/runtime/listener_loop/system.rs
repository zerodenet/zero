use std::future::Future;

use tokio::net::TcpStream;
use tokio::sync::watch;
use tokio::task::JoinSet;
use tracing::{error, info};
use zero_stack::SystemTcpStack;
use zero_traits::TcpStack;

use crate::runtime::Proxy;

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
