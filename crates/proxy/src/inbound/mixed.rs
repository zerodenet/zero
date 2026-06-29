use socks5::Socks5Request;
use tokio::sync::watch;
use tokio::task::JoinSet;
use tracing::{error, info};
use zero_core::Error as CoreError;
use zero_engine::EngineError;
use zero_traits::AsyncSocket;

use crate::logging::log_listener_connection_error;
use crate::runtime::inbound_protocol::serve_inbound;
use crate::runtime::Proxy;
use crate::transport::{MeteredStream, PrefixedSocket, TcpRelayStream};

use super::http_connect::HttpConnectInboundHandler;
use super::socks5::Socks5InboundHandler;

pub(crate) struct MixedInboundRequest {
    pub(crate) inbound: zero_config::InboundConfig,
    pub(crate) socks5_auth: socks5::ConfiguredSocks5PasswordAuth,
}

pub(crate) async fn run_mixed_listener_with_bound(
    proxy: &Proxy,
    request: MixedInboundRequest,
    listener: zero_platform_tokio::TokioListener,
    mut shutdown: watch::Receiver<bool>,
) -> Result<(), EngineError> {
    let MixedInboundRequest {
        inbound,
        socks5_auth,
    } = request;
    let local_addr = listener.local_addr()?;
    let mut connections = JoinSet::new();

    let socks5_handler = Socks5InboundHandler::new(socks5::Socks5Inbound, socks5_auth);
    let http_handler = HttpConnectInboundHandler {
        http_connect_inbound: http_connect::HttpConnectInbound,
    };

    info!(
        inbound_tag = %inbound.tag,
        protocol = "mixed",
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
                    Ok((mut stream, remote_addr)) => {
                        let engine = proxy.clone();
                        let tag = inbound.tag.clone();
                        let socks5_h = socks5_handler.clone();
                        let http_h = http_handler.clone();
                        let source_addr = remote_addr_to_socket(remote_addr);
                        connections.spawn(async move {
                            // Detect protocol from first byte
                            let mut first = [0_u8; 1];
                            if stream.read(&mut first).await.map_or(true, |n| n == 0) {
                                return;
                            }
                            let first_byte = first[0];
                            let relay_stream = prefixed_relay_stream(stream, first_byte);

                            if first_byte == 0x05 {
                                // SOCKS5
                                let mut metered = MeteredStream::new(
                                    relay_stream,
                                );
                                match socks5_h.accept_command(&mut metered).await
                                {
                                    Ok(Socks5Request::Connect(session)) => {
                                        let _ = serve_inbound(
                                            &engine, *session, metered.into_inner(),
                                            &socks5_h, &tag, source_addr,
                                        ).await;
                                    }
                                    Ok(Socks5Request::UdpAssociate(request)) => {
                                        let _ = engine.handle_socks5_udp_associate(
                                            metered, &tag, request,
                                        ).await;
                                    }
                                    Err(err) => {
                                        let engine_err = EngineError::from(err);
                                        log_listener_connection_error(
                                            "mixed", &tag, &source_addr, &engine_err,
                                        );
                                    }
                                }
                            } else {
                                // HTTP CONNECT
                                let mut metered = MeteredStream::new(
                                    relay_stream,
                                );
                                match http_h.http_connect_inbound()
                                    .accept_request(&mut metered).await
                                {
                                    Ok(session) => {
                                        let _ = serve_inbound(
                                            &engine, session, metered.into_inner(),
                                            &http_h, &tag, source_addr,
                                        ).await;
                                    }
                                    Err(CoreError::Unsupported(_)) => {
                                        let _ = http_h.http_connect_inbound()
                                            .send_response(
                                                &mut metered,
                                                http_connect::HttpConnectResponse::MethodNotAllowed,
                                            ).await;
                                    }
                                    Err(CoreError::Protocol(_)) => {
                                        let _ = http_h.http_connect_inbound()
                                            .send_response(
                                                &mut metered,
                                                http_connect::HttpConnectResponse::BadRequest,
                                            ).await;
                                    }
                                    Err(err) => {
                                        let engine_err = EngineError::from(err);
                                        log_listener_connection_error(
                                            "mixed", &tag, &source_addr, &engine_err,
                                        );
                                    }
                                }
                            }
                        });
                    }
                    Err(e) => {
                        error!(error = %e, "mixed: accept error");
                    }
                }
            }
            result = connections.join_next(), if !connections.is_empty() => {
                if let Some(Err(error)) = result {
                    if !error.is_cancelled() {
                        error!(error = %error, "mixed connection task panicked");
                    }
                }
            }
        }
    }

    connections.abort_all();
    while let Some(result) = connections.join_next().await {
        if let Err(error) = result {
            if !error.is_cancelled() {
                error!(error = %error, "mixed connection task panicked during shutdown");
            }
        }
    }

    info!(inbound_tag = %inbound.tag, protocol = "mixed", listen = %local_addr, "inbound listener stopped");
    Ok(())
}

fn remote_addr_to_socket(addr: Option<zero_traits::IpAddress>) -> Option<std::net::SocketAddr> {
    addr.map(|ip| match ip {
        zero_traits::IpAddress::V4(octets) => {
            std::net::SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::from(octets)), 0)
        }
        zero_traits::IpAddress::V6(octets) => {
            std::net::SocketAddr::new(std::net::IpAddr::V6(std::net::Ipv6Addr::from(octets)), 0)
        }
    })
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
