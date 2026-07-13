use zero_config::InboundConfig;
use zero_engine::EngineError;
use zero_transport::trojan_transport::TrojanInboundListenerRequest;

use crate::runtime::inbound_route::dispatch_no_client_stream_route;
use crate::runtime::listener_loop::{
    run_logged_tcp_socket_listener_loop, LoggedTcpSocketListenerRequest,
};

pub(super) fn prepare(
    inbound: InboundConfig,
    source_dir: Option<&std::path::Path>,
) -> Result<Box<dyn crate::runtime::inbound_operation::PreparedInboundListenerOperation>, EngineError>
{
    let request =
        TrojanInboundListenerRequest::from_protocol_config(&inbound.protocol, source_dir)?;
    Ok(Box::new(
        crate::runtime::inbound_operation::InboundListenerOperation::new(
            move |proxy, bound: crate::protocol_registry::BoundInbound, shutdown_rx| async move {
                let protocol_name = request.protocol_name();
                let error_protocol_name = request.error_protocol_name();

                run_logged_tcp_socket_listener_loop(LoggedTcpSocketListenerRequest {
                    proxy: &proxy,
                    inbound_tag: inbound.tag,
                    protocol_name,
                    error_protocol_name,
                    request,
                    listener: bound.into_tcp(),
                    shutdown: shutdown_rx,
                    dispatch: |proxy,
                               request: TrojanInboundListenerRequest,
                               inbound_tag,
                               socket,
                               source_addr| async move {
                        let defaults = request.no_client_stream_route_defaults();
                        let route = request.accept_route(socket).await?;
                        dispatch_no_client_stream_route(
                            route,
                            proxy,
                            inbound_tag,
                            source_addr,
                            defaults.udp_protocol,
                        )
                        .await
                    },
                })
                .await
            },
        ),
    ))
}
