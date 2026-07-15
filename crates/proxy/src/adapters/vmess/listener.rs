use ::vmess::transport::VmessInboundListenerRequest;

use crate::runtime::inbound_operation::TcpInboundListenerOperation;
use crate::runtime::inbound_route::NoClientMuxRouteDefaults;

pub(super) fn prepare(
    request: VmessInboundListenerRequest,
) -> Box<dyn crate::runtime::inbound_operation::PreparedInboundListenerOperation> {
    Box::new(TcpInboundListenerOperation {
        protocol_name: request.protocol_name(),
        error_protocol_name: request.error_protocol_name(),
        request,
        dispatch:
            |request: VmessInboundListenerRequest,
             socket,
             context: crate::runtime::inbound_operation::InboundConnectionContext| async move {
                let defaults: NoClientMuxRouteDefaults =
                    request.no_client_mux_route_defaults().into();
                let route = request.accept_route(socket).await?;
                context.dispatch_no_client_mux_route(route, defaults).await
            },
    })
}
