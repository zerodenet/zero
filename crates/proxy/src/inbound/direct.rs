//! Direct inbound - fixed-target forwarder.
//!
//! Listens on a port, accepts raw TCP connections with no protocol
//! handshake, and forwards all traffic through the kernel pipeline
//! to a configured outbound (node or group).

use async_trait::async_trait;
use tokio::sync::watch;
use zero_core::{Address, Network, ProtocolType, Session};
use zero_engine::EngineError;

use crate::runtime::inbound_protocol::{serve_inbound, InboundProtocol};
use crate::runtime::listener_loop::{run_tcp_listener_loop, TcpListenerLoopRequest};
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;

#[derive(Debug)]
pub(crate) struct DirectInboundRequest {
    pub(crate) inbound: zero_config::InboundConfig,
    pub(crate) target: Option<String>,
    pub(crate) port: Option<u16>,
}

#[derive(Clone)]
pub(crate) struct DirectInboundHandler;

#[async_trait]
impl InboundProtocol for DirectInboundHandler {
    type ClientStream = TcpRelayStream;

    async fn send_ok(&self, _: &mut TcpRelayStream) -> Result<(), EngineError> {
        Ok(())
    }

    async fn send_blocked(&self, _: &mut TcpRelayStream) -> Result<(), EngineError> {
        Ok(())
    }

    async fn send_upstream_failure(&self, _: &mut TcpRelayStream) -> Result<(), EngineError> {
        Ok(())
    }
}

pub(crate) async fn run_direct_listener_with_bound(
    proxy: &Proxy,
    request: DirectInboundRequest,
    listener: zero_platform_tokio::TokioListener,
    shutdown: watch::Receiver<bool>,
) -> Result<(), EngineError> {
    let DirectInboundRequest {
        inbound,
        target,
        port,
    } = request;
    let handler = DirectInboundHandler;

    run_tcp_listener_loop(TcpListenerLoopRequest {
        proxy,
        inbound_tag: inbound.tag,
        protocol_name: "direct",
        listener,
        shutdown,
        handler: move |engine: Proxy,
                       tag: String,
                       stream: zero_platform_tokio::TokioSocket,
                       source_addr: Option<std::net::SocketAddr>| {
            let target = target.clone();
            let handler = handler.clone();
            async move {
                let address = match target.as_deref() {
                    Some(value) if value.parse::<std::net::Ipv4Addr>().is_ok() => {
                        Address::Ipv4(value.parse::<std::net::Ipv4Addr>().unwrap().octets())
                    }
                    Some(value) if value.parse::<std::net::Ipv6Addr>().is_ok() => {
                        Address::Ipv6(value.parse::<std::net::Ipv6Addr>().unwrap().octets())
                    }
                    Some(value) => Address::Domain(value.to_owned()),
                    None => return,
                };
                let session = Session::new(
                    0,
                    address,
                    port.unwrap_or(443),
                    Network::Tcp,
                    ProtocolType::Unknown,
                );
                let _ = serve_inbound(
                    &engine,
                    session,
                    TcpRelayStream::from(stream),
                    &handler,
                    &tag,
                    source_addr,
                )
                .await;
            }
        },
    })
    .await
}
