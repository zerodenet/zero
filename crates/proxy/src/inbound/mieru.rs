//! Mieru inbound encrypted handshake and AEAD-framed relay.

use std::net::SocketAddr;

use async_trait::async_trait;
use mieru::{MieruInbound, MieruInboundProfile};
use tokio::select;
use tokio::sync::watch;
use tokio::task::JoinSet;
use tokio::time::Instant as TokioInstant;
use tracing::{error, info};
use zero_config::InboundConfig;
use zero_core::Session;
use zero_engine::EngineError;

use crate::logging::log_listener_connection_error;
use crate::runtime::inbound_protocol::{serve_inbound, InboundProtocol};
use crate::runtime::pipe::{KernelPipe, UdpPipe, UdpPipeInput};
use crate::runtime::udp_flow::helpers::{
    log_completed_udp_flow, record_udp_inbound_response_rx, record_udp_inbound_response_tx,
    udp_response_session_id, wait_for_upstream_idle,
};
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;

type MieruClientStream = mieru::MieruInboundStream<TcpRelayStream>;

#[derive(Debug)]
pub(crate) struct MieruInboundRequest {
    pub(crate) inbound: InboundConfig,
    pub(crate) profile: MieruInboundProfile,
}

// Handler.

#[derive(Clone)]
pub(crate) struct MieruInboundHandler {
    mieru_inbound: MieruInbound,
    profile: MieruInboundProfile,
}

#[async_trait]
impl InboundProtocol for MieruInboundHandler {
    type ClientStream = MieruClientStream;

    async fn accept(
        &self,
        stream: TcpRelayStream,
    ) -> Result<(Session, Self::ClientStream), EngineError> {
        let mut metered = crate::transport::MeteredStream::new(stream);
        let accept = self
            .profile
            .accept_request(&self.mieru_inbound, &mut metered)
            .await?;

        let mut client = mieru::MieruInboundStream::new(metered.into_inner(), accept);

        let mut session = client.accept_tunneled_socks5_session().await?;
        session.apply_auth(self.profile.inbound_auth());

        Ok((session, client))
    }

    async fn send_ok(&self, _client: &mut Self::ClientStream) -> Result<(), EngineError> {
        Ok(()) // Mieru handshake already confirms success
    }

    async fn send_blocked(&self, _client: &mut Self::ClientStream) -> Result<(), EngineError> {
        // Mieru protocol has no explicit blocked response;
        // the connection close serves as the signal.
        Ok(())
    }

    async fn send_upstream_failure(
        &self,
        _client: &mut Self::ClientStream,
    ) -> Result<(), EngineError> {
        self.send_blocked(_client).await
    }
}

// Listener.

pub(crate) async fn run_mieru_listener_with_bound(
    proxy: &Proxy,
    request: MieruInboundRequest,
    listener: zero_platform_tokio::TokioListener,
    mut shutdown: watch::Receiver<bool>,
) -> Result<(), EngineError> {
    let MieruInboundRequest { inbound, profile } = request;
    let local_addr = listener.local_addr()?;

    let handler = MieruInboundHandler {
        mieru_inbound: MieruInbound,
        profile,
    };

    let mut connections: JoinSet<Result<(), EngineError>> = JoinSet::new();

    info!(
        inbound_tag = %inbound.tag,
        protocol = "mieru",
        listen = %local_addr,
        "inbound listener ready"
    );

    loop {
        select! {
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
                        let tag = inbound.tag.clone();
                        let handler = handler.clone();
                        let source_addr = remote_addr_to_socket(remote_addr);
                        connections.spawn(async move {
                            match handler.accept(stream.into()).await {
                                Ok((session, client)) => {
                                    if session.network == zero_core::Network::Udp {
                                        let _ = engine.run_mieru_udp_relay(
                                            client, &session, &tag,
                                        ).await;
                                    } else {
                                        let _ = serve_inbound(
                                            &engine, session, client, &handler,
                                            &tag, source_addr,
                                        ).await;
                                    }
                                }
                                Err(error) => {
                                    log_listener_connection_error(
                                        "mieru", &tag, &remote_addr, &error,
                                    );
                                }
                            }
                            Ok(())
                        });
                    }
                    Err(e) => {
                        error!(error = %e, "mieru: accept error");
                        break;
                    }
                }
            }
            result = connections.join_next(), if !connections.is_empty() => {
                match result {
                    Some(Err(error)) if !error.is_cancelled() => {
                        error!(error = %error, "mieru connection task panicked");
                    }
                    _ => {}
                }
            }
        }
    }

    connections.abort_all();
    while let Some(result) = connections.join_next().await {
        if let Err(error) = result {
            if !error.is_cancelled() {
                error!(error = %error, "mieru shutdown error");
            }
        }
    }

    info!(inbound_tag = %inbound.tag, protocol = "mieru", "listener stopped");
    Ok(())
}

// UDP relay.

impl Proxy {
    /// Run a Mieru UDP relay through the generic UDP pipe.
    async fn run_mieru_udp_relay(
        &self,
        mut client: MieruClientStream,
        session: &Session,
        inbound_tag: &str,
    ) -> Result<(), EngineError> {
        let mut dispatch = crate::runtime::udp_dispatch::UdpDispatch::new(inbound_tag).await?;
        let auth = session.auth.clone();
        let mut last_activity = TokioInstant::now();
        let timeout = self.udp_upstream_idle_timeout();

        let mut read_buf = [0u8; 65536];
        let mut direct_buf = [0u8; 65536];
        let mut upstream_buf = [0u8; 65536];
        let udp_session = mieru::MieruInbound.udp_session();

        info!(
            inbound_tag = inbound_tag,
            protocol = "mieru_udp",
            "mieru udp session started"
        );

        loop {
            let (direct_sock, upstream_udp, socks5_idle, chain_tasks) = dispatch.poll_refs();

            tokio::select! {
                _ = tokio::time::sleep_until(last_activity + timeout) => {
                    info!(
                        inbound_tag = inbound_tag,
                        protocol = "mieru_udp",
                        "mieru udp session idle timeout"
                    );
                    break;
                }
                read = udp_session.read_dispatch_view_tokio(&mut client, &mut read_buf) => {
                    match read {
                        Ok(None) => break,
                        Ok(Some(dispatch_view)) => {
                            last_activity = TokioInstant::now();
                            let (target, port, payload, client_session_id) =
                                dispatch_view.into_pipe_parts();
                            if let Err(error) = UdpPipe::new(self, &mut dispatch)
                                .dispatch(UdpPipeInput {
                                    target,
                                    port,
                                    payload: &payload,
                                    protocol: zero_core::ProtocolType::Mieru,
                                    auth: auth.as_ref(),
                                    client_session_id,
                                })
                                .await
                            {
                                tracing::warn!(error = %error, "failed to process mieru udp packet");
                            }
                        }
                        Err(error) => {
                            tracing::warn!(error = %error, "mieru udp request read/decode failed");
                            break;
                        }
                    }
                }
                recv = direct_sock.recv_from_addr(&mut direct_buf) => {
                    let (n, sender) = recv?;
                    last_activity = TokioInstant::now();

                    let session_id = dispatch.direct_response_session_id(sender);
                    record_udp_inbound_response_rx(self, session_id, n);
                    let written = udp_session
                        .write_response_for_sender_tokio(&mut client, sender, &direct_buf[..n])
                        .await?;
                    record_udp_inbound_response_tx(self, session_id, written);
                }
                upstream = upstream_udp.recv_response(&mut upstream_buf) => {
                    match upstream {
                        Ok(pkt) => {
                            last_activity = TokioInstant::now();
                            self.record_udp_upstream_packet_received();
                            dispatch.touch_upstream_idle(self.udp_upstream_idle_timeout());
                            let (target, port, payload) = pkt.into_parts();
                            let session_id = udp_response_session_id(&dispatch, &target, port);
                            record_udp_inbound_response_rx(self, session_id, payload.len());
                            let written = udp_session
                                .write_response_for_target_tokio(&mut client, &target, port, &payload)
                                .await?;
                            record_udp_inbound_response_tx(self, session_id, written);
                        }
                        Err(error) => {
                            tracing::warn!(error = %error, "mieru udp socks5 upstream recv error");
                        }
                    }
                }
                _ = wait_for_upstream_idle(socks5_idle) => {}
                Some(chain_result) = chain_tasks.join_next() => {
                    match chain_result {
                        Ok(Ok((target, port, payload, session_id))) => {
                            last_activity = TokioInstant::now();
                            record_udp_inbound_response_rx(self, session_id, payload.len());
                            let written = udp_session
                                .write_response_for_target_tokio(&mut client, &target, port, &payload)
                                .await?;
                            record_udp_inbound_response_tx(self, session_id, written);
                        }
                        Ok(Err(error)) => tracing::warn!(error = %error, "mieru udp chain response error"),
                        Err(error) => tracing::warn!(error = %error, "mieru udp chain task panicked"),
                    }
                }
            }
        }

        for completed in dispatch.finish_all() {
            log_completed_udp_flow(completed);
        }

        tracing::info!(inbound_tag = %inbound_tag, "mieru udp session ended");
        Ok(())
    }
}

fn remote_addr_to_socket(addr: Option<zero_traits::IpAddress>) -> Option<SocketAddr> {
    addr.map(|ip| match ip {
        zero_traits::IpAddress::V4(octets) => {
            SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::from(octets)), 0)
        }
        zero_traits::IpAddress::V6(octets) => {
            SocketAddr::new(std::net::IpAddr::V6(std::net::Ipv6Addr::from(octets)), 0)
        }
    })
}
