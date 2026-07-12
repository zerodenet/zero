use tokio::sync::watch;
use zero_config::InboundConfig;
use zero_engine::EngineError;

use crate::logging::log_listener_connection_error;
use crate::runtime::inbound_protocol::serve_inbound_with_client_response;
use crate::runtime::listener_loop::{run_tcp_listener_loop, TcpListenerLoopRequest};
use crate::runtime::Proxy;
use crate::transport::{MeteredStream, TcpRelayStream};

#[path = "udp_associate.rs"]
pub(crate) mod udp_associate;

pub(crate) async fn handle_socks5_connection(
    proxy: &Proxy,
    inbound_tag: &str,
    source_addr: Option<std::net::SocketAddr>,
    metered: MeteredStream<TcpRelayStream>,
    acceptor: &zero_transport::socks5_transport::OwnedSocks5InboundAcceptor,
    protocol_name: &'static str,
) {
    match acceptor
        .accept_and_dispatch_command(
            metered,
            |session, stream| async move {
                serve_inbound_with_client_response(
                    proxy,
                    session,
                    stream,
                    acceptor.clone(),
                    inbound_tag,
                    source_addr,
                )
                .await
            },
            |setup, stream| async move {
                udp_associate::run_socks5_udp_associate(proxy, stream, inbound_tag, setup).await
            },
        )
        .await
    {
        Ok(()) => {}
        Err(err) => {
            log_listener_connection_error(protocol_name, inbound_tag, &source_addr, &err);
        }
    }
}

pub(crate) async fn run_socks5_listener_with_bound(
    proxy: &Proxy,
    inbound: InboundConfig,
    acceptor: zero_transport::socks5_transport::OwnedSocks5InboundAcceptor,
    listener: zero_platform_tokio::TokioListener,
    shutdown: watch::Receiver<bool>,
) -> Result<(), EngineError> {
    run_tcp_listener_loop(TcpListenerLoopRequest {
        proxy,
        inbound_tag: inbound.tag,
        protocol_name: "socks5",
        listener,
        shutdown,
        handler: move |engine: Proxy,
                       tag: String,
                       stream: zero_platform_tokio::TokioSocket,
                       source_addr: Option<std::net::SocketAddr>| {
            let acceptor = acceptor.clone();
            async move {
                handle_socks5_connection(
                    &engine,
                    &tag,
                    source_addr,
                    MeteredStream::new(TcpRelayStream::from(stream)),
                    &acceptor,
                    "socks5",
                )
                .await;
            }
        },
    })
    .await
}
