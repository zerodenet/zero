use ::vless::transport::VlessInboundListenerRequest;
use zero_engine::EngineError;

use crate::runtime::inbound_operation::{
    InboundConnectionContext, TcpOrQuicInboundListenerOperation,
};
use crate::runtime::inbound_route::RecordedProtocolMuxRouteDefaults;
use crate::runtime::tcp_ingress::ClientResponseInboundProtocol;

pub(super) fn prepare(
    request: VlessInboundListenerRequest,
) -> Box<dyn crate::runtime::inbound_operation::PreparedInboundListenerOperation> {
    Box::new(TcpOrQuicInboundListenerOperation {
        protocol_name: request.protocol_name(),
        error_protocol_name: request.error_protocol_name(),
        request,
        dispatch_tcp: |request: VlessInboundListenerRequest,
                       socket,
                       context: InboundConnectionContext| async move {
            let protocol = ClientResponseInboundProtocol::new(request.response_protocol());
            let defaults: RecordedProtocolMuxRouteDefaults =
                request.recorded_mux_route_defaults().into();
            context
                .dispatch_recorded_mux_tcp_route(
                    request
                        .accept_recorded_tcp_route(socket)
                        .await
                        .map_err(EngineError::from),
                    protocol,
                    defaults,
                )
                .await
        },
        dispatch_quic: |request: VlessInboundListenerRequest,
                        stream,
                        context: InboundConnectionContext| async move {
            let protocol = ClientResponseInboundProtocol::new(request.response_protocol());
            let defaults: RecordedProtocolMuxRouteDefaults =
                request.recorded_mux_route_defaults().into();
            context
                .dispatch_recorded_mux_stream_route(
                    request
                        .accept_recorded_stream_route(stream)
                        .await
                        .map_err(EngineError::from),
                    protocol,
                    defaults,
                )
                .await
        },
    })
}
