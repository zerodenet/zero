use zero_config::InboundConfig;
use zero_engine::EngineError;
use zero_transport::vmess_transport::VmessInboundListenerRequest;

use crate::runtime::inbound_route::{
    dispatch_no_client_mux_route_request_with_defaults, NoClientMuxRouteDefaults,
};
use crate::runtime::listener_loop::{
    run_logged_tcp_socket_listener_loop, LoggedTcpSocketListenerRequest,
};

pub(super) fn prepare(
    inbound: InboundConfig,
    source_dir: Option<&std::path::Path>,
) -> Result<Box<dyn crate::runtime::inbound_operation::PreparedInboundListenerOperation>, EngineError>
{
    let request = VmessInboundListenerRequest::from_protocol_config(&inbound.protocol, source_dir)?;
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
                               request: VmessInboundListenerRequest,
                               inbound_tag,
                               socket,
                               source_addr| async move {
                        let defaults: NoClientMuxRouteDefaults =
                            request.no_client_mux_route_defaults().into();
                        let route = request.accept_route(socket).await?;
                        dispatch_no_client_mux_route_request_with_defaults(
                            route,
                            proxy,
                            inbound_tag,
                            source_addr,
                            defaults,
                        )
                        .await
                    },
                })
                .await
            },
        ),
    ))
}
