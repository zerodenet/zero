use ::trojan::transport::TrojanInboundListenerRequest;

use crate::runtime::inbound_operation::TcpInboundListenerOperation;

pub(super) fn prepare(
    request: TrojanInboundListenerRequest,
) -> Box<dyn crate::runtime::inbound_operation::PreparedInboundListenerOperation> {
    Box::new(TcpInboundListenerOperation {
        protocol_name: request.protocol_name(),
        error_protocol_name: request.error_protocol_name(),
        request,
        dispatch:
            |request: TrojanInboundListenerRequest,
             socket,
             context: crate::runtime::inbound_operation::InboundConnectionContext| async move {
                let defaults = request.no_client_stream_route_defaults();
                let route = request.accept_route(socket).await?;
                context
                    .dispatch_no_client_stream_route(route, defaults.udp_protocol)
                    .await
            },
    })
}
