//! Mieru listener lifecycle and post-accept runtime handoff.

use tokio::sync::watch;
use zero_config::InboundConfig;
use zero_engine::EngineError;

use crate::logging::log_listener_connection_error;
use crate::runtime::listener_loop::{run_tcp_listener_loop, TcpListenerLoopRequest};
use crate::runtime::stream_udp::run_mapped_protocol_stream_udp_relay;
use crate::runtime::tcp_ingress::serve_inbound_with_client_response;
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
                run_mapped_protocol_stream_udp_relay(
                    proxy,
                    &session,
                    relay,
                    inbound_tag,
                    "mieru_udp",
                    core::convert::identity,
                    None,
                )
                .await
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
                        log_listener_connection_error(
                            crate::logging::INBOUND_ACCEPT_ROUTE_STAGE,
                            "mieru",
                            &tag,
                            &source_addr,
                            &error,
                        );
                    }
                }
            }
        },
    })
    .await
}

impl crate::adapters::mieru::MieruAdapter {
    pub(super) fn prepare_inbound_listener_impl(
        &self,
        inbound: zero_config::InboundConfig,
    ) -> Result<
        Box<dyn crate::runtime::inbound_operation::PreparedInboundListenerOperation>,
        EngineError,
    > {
        let profile =
            zero_transport::mieru_transport::inbound_profile_from_protocol(&inbound.protocol)?;
        Ok(Box::new(
            crate::runtime::inbound_operation::InboundListenerOperation::new(
                move |proxy, bound: crate::protocol_registry::BoundInbound, shutdown_rx| async move {
                    run_mieru_listener_with_bound(
                        &proxy,
                        inbound,
                        profile,
                        bound.into_tcp(),
                        shutdown_rx,
                    )
                    .await
                },
            ),
        ))
    }
}
