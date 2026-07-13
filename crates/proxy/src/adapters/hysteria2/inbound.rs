//! Hysteria2 listener lifecycle and post-accept runtime handoff.

use std::io;
use tokio::sync::watch;
use tracing::{error, warn};
use zero_config::InboundConfig;
use zero_engine::EngineError;

use crate::runtime::datagram_udp::run_protocol_datagram_udp_relay;
use crate::runtime::listener_loop::{run_quic_listener_loop, QuicListenerLoopRequest};
use crate::runtime::tcp_ingress::serve_inbound_with_client_response;
use crate::runtime::Proxy;

// Listener (QUIC connection lifecycle).

pub(crate) async fn run_hysteria2_listener_with_bound(
    proxy: &Proxy,
    inbound: InboundConfig,
    profile: zero_transport::hysteria2_quic::OwnedHysteria2InboundProfile,
    bound: crate::protocol_registry::BoundInbound,
    shutdown: watch::Receiver<bool>,
) -> Result<(), EngineError> {
    let quic_inbound = match bound {
        crate::protocol_registry::BoundInbound::Quic(e) => e,
        _ => {
            return Err(EngineError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "hysteria2 listener requires QUIC transport",
            )))
        }
    };

    let acceptor = profile.tcp_response_protocol();

    run_quic_listener_loop(QuicListenerLoopRequest {
        proxy,
        inbound_tag: inbound.tag,
        protocol_name: "hysteria2",
        listener: quic_inbound,
        shutdown,
        handler: move |engine: Proxy, tag: String, conn: quinn::Connection| {
            let profile = profile.clone();
            let acceptor = acceptor;
            async move {
                if let Err(error) =
                    handle_hysteria2_connection(&engine, conn, &tag, profile, acceptor).await
                {
                    error!(error = %error, "hysteria2 connection error");
                }
            }
        },
    })
    .await
}

/// Handle a single Hysteria2 QUIC connection.
async fn handle_hysteria2_connection(
    proxy: &Proxy,
    conn: quinn::Connection,
    inbound_tag: &str,
    profile: zero_transport::hysteria2_quic::OwnedHysteria2InboundProfile,
    acceptor: zero_transport::hysteria2_quic::OwnedHysteria2InboundTcpResponseProtocol,
) -> Result<(), EngineError> {
    zero_transport::hysteria2_quic::accept_and_dispatch_authenticated_hysteria2_quic_session(
        &profile,
        conn,
        |conn, relay, tasks| {
            let tag = inbound_tag.to_owned();
            let engine = proxy.clone();
            tasks.spawn(async move {
                let response_already_sent = true;
                run_protocol_datagram_udp_relay(&engine, conn, relay, &tag, response_already_sent)
                    .await
            });
            std::future::ready(Ok(()))
        },
        |session, stream, tasks| {
            let engine = proxy.clone();
            let tag = inbound_tag.to_owned();
            tasks.spawn(async move {
                let _ = serve_inbound_with_client_response(
                    &engine, session, stream, acceptor, &tag, None,
                )
                .await;
                Ok(())
            });
            std::future::ready(Ok(()))
        },
        |result| async move {
            match result {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    warn!(error = %error, "hysteria2 stream task failed");
                }
                Err(error) if !error.is_cancelled() => {
                    error!(error = %error, "hysteria2 stream task panicked");
                }
                Err(_) => {}
            }
            Ok(())
        },
    )
    .await
}

impl crate::adapters::hysteria2::Hysteria2Adapter {
    pub(super) fn prepare_inbound_listener_impl(
        &self,
        inbound: zero_config::InboundConfig,
    ) -> Result<
        Box<dyn crate::runtime::inbound_operation::PreparedInboundListenerOperation>,
        EngineError,
    > {
        let profile =
            zero_transport::hysteria2_quic::inbound_profile_from_protocol(&inbound.protocol)?;
        Ok(Box::new(
            crate::runtime::inbound_operation::InboundListenerOperation::new(
                move |proxy, bound: crate::protocol_registry::BoundInbound, shutdown_rx| async move {
                    run_hysteria2_listener_with_bound(&proxy, inbound, profile, bound, shutdown_rx)
                        .await
                },
            ),
        ))
    }
}
