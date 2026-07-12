//! Mieru inbound encrypted handshake and AEAD-framed relay.

#[path = "udp.rs"]
mod udp;

use tokio::sync::watch;
use zero_config::InboundConfig;
use zero_engine::EngineError;

use crate::logging::log_listener_connection_error;
use crate::runtime::inbound_protocol::serve_inbound_with_client_response;
use crate::runtime::listener_loop::{run_tcp_listener_loop, TcpListenerLoopRequest};
use crate::runtime::Proxy;
use crate::transport::{MeteredStream, TcpRelayStream};

async fn handle_mieru_connection(
    proxy: &Proxy,
    profile: &zero_transport::mieru_transport::OwnedMieruInboundProfile,
    inbound_tag: &str,
    source_addr: Option<std::net::SocketAddr>,
    stream: TcpRelayStream,
) -> Result<(), EngineError> {
    let metered = MeteredStream::new(stream);
    profile
        .accept_and_dispatch_client(
            metered,
            |session, stream| async move {
                serve_inbound_with_client_response(
                    proxy,
                    session,
                    stream,
                    profile.response_protocol(),
                    inbound_tag,
                    source_addr,
                )
                .await
            },
            |session, relay| async move {
                udp::run_mieru_udp_relay(proxy, relay, &session, inbound_tag).await
            },
        )
        .await
}

pub(crate) async fn run_mieru_listener_with_bound(
    proxy: &Proxy,
    inbound: InboundConfig,
    profile: zero_transport::mieru_transport::OwnedMieruInboundProfile,
    listener: zero_platform_tokio::TokioListener,
    shutdown: watch::Receiver<bool>,
) -> Result<(), EngineError> {
    run_tcp_listener_loop(TcpListenerLoopRequest {
        proxy,
        inbound_tag: inbound.tag,
        protocol_name: "mieru",
        listener,
        shutdown,
        handler: move |engine: Proxy,
                       tag: String,
                       stream: zero_platform_tokio::TokioSocket,
                       source_addr: Option<std::net::SocketAddr>| {
            let profile = profile.clone();
            async move {
                match handle_mieru_connection(
                    &engine,
                    &profile,
                    tag.as_str(),
                    source_addr,
                    stream.into(),
                )
                .await
                {
                    Ok(()) => {}
                    Err(error) => {
                        log_listener_connection_error("mieru", &tag, &source_addr, &error);
                    }
                }
            }
        },
    })
    .await
}
