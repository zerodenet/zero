use tokio::sync::watch;
use zero_engine::EngineError;
use zero_traits::AsyncSocket;

use crate::adapters::http_connect::inbound::HttpConnectInboundHandler;
use crate::adapters::socks5::inbound::handle_socks5_connection;
use crate::logging::log_listener_connection_error;
use crate::runtime::inbound_protocol::serve_inbound;
use crate::runtime::listener_loop::{run_tcp_listener_loop, TcpListenerLoopRequest};
use crate::runtime::Proxy;
use crate::transport::{MeteredStream, PrefixedSocket, TcpRelayStream};

pub(crate) struct MixedInboundRequest {
    pub(crate) inbound_tag: String,
    pub(crate) socks5_acceptor: zero_transport::socks5_transport::OwnedSocks5InboundAcceptor,
}

pub(crate) async fn run_mixed_listener_with_bound(
    proxy: &Proxy,
    request: MixedInboundRequest,
    listener: zero_platform_tokio::TokioListener,
    shutdown: watch::Receiver<bool>,
) -> Result<(), EngineError> {
    let MixedInboundRequest {
        inbound_tag,
        socks5_acceptor,
    } = request;

    let http_handler = HttpConnectInboundHandler::default();

    run_tcp_listener_loop(TcpListenerLoopRequest {
        proxy,
        inbound_tag,
        protocol_name: "mixed",
        listener,
        shutdown,
        handler: move |engine, tag, stream, source_addr| {
            let socks5_acceptor = socks5_acceptor.clone();
            async move {
                handle_mixed_connection(
                    engine,
                    tag,
                    stream,
                    source_addr,
                    socks5_acceptor,
                    http_handler,
                )
                .await;
            }
        },
    })
    .await
}

async fn handle_mixed_connection(
    engine: Proxy,
    tag: String,
    mut stream: zero_platform_tokio::TokioSocket,
    source_addr: Option<std::net::SocketAddr>,
    socks5_acceptor: zero_transport::socks5_transport::OwnedSocks5InboundAcceptor,
    http_h: HttpConnectInboundHandler,
) {
    // Detect protocol from first byte.
    let mut first = [0_u8; 1];
    if stream.read(&mut first).await.map_or(true, |n| n == 0) {
        return;
    }
    let first_byte = first[0];
    let relay_stream = prefixed_relay_stream(stream, first_byte);

    if socks5::is_socks5_greeting_byte(first_byte) {
        handle_socks5_connection(
            &engine,
            &tag,
            source_addr,
            MeteredStream::new(relay_stream),
            &socks5_acceptor,
            "mixed",
        )
        .await;
    } else {
        let mut metered = MeteredStream::new(relay_stream);
        match http_h
            .http_connect_inbound()
            .accept_request(&mut metered)
            .await
        {
            Ok(session) => {
                let _ = serve_inbound(
                    &engine,
                    session,
                    metered.into_inner(),
                    &http_h,
                    &tag,
                    source_addr,
                )
                .await;
            }
            Err(err) => {
                if http_h
                    .http_connect_inbound()
                    .send_accept_error_response(&mut metered, &err)
                    .await
                    .unwrap_or(false)
                {
                    return;
                }
                let engine_err = EngineError::from(err);
                log_listener_connection_error("mixed", &tag, &source_addr, &engine_err);
            }
        }
    }
}

fn prefixed_relay_stream(
    stream: zero_platform_tokio::TokioSocket,
    first_byte: u8,
) -> TcpRelayStream {
    let local_addr = stream.local_addr().ok();
    let prefixed = PrefixedSocket::from_byte(stream, first_byte);

    match local_addr {
        Some(addr) => TcpRelayStream::with_local_addr(prefixed, addr),
        None => TcpRelayStream::new(prefixed),
    }
}
