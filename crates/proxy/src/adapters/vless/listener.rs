use ::vless::transport::VlessInboundListenerRequest;
use zero_engine::EngineError;

use crate::runtime::inbound_operation::{
    InboundConnectionContext, TcpOrQuicInboundListenerOperation,
};
use crate::runtime::inbound_route::RecordedProtocolMuxRouteDefaults;
use crate::runtime::tcp_ingress::ClientResponseInboundProtocol;
use crate::runtime::{
    prepare_inbound_route_accept, InboundFallbackTarget, PreparedInboundRouteAccept,
};

#[derive(Clone)]
struct PreparedVlessInboundRequest {
    protocol: VlessInboundListenerRequest,
    fallback: Option<InboundFallbackTarget>,
}

pub(super) fn prepare(
    request: VlessInboundListenerRequest,
    fallback: Option<InboundFallbackTarget>,
) -> Box<dyn crate::runtime::inbound_operation::PreparedInboundListenerOperation> {
    let request = PreparedVlessInboundRequest {
        protocol: request,
        fallback,
    };
    Box::new(TcpOrQuicInboundListenerOperation {
        protocol_name: request.protocol.protocol_name(),
        error_protocol_name: request.protocol.error_protocol_name(),
        request,
        dispatch_tcp: |request: PreparedVlessInboundRequest,
                       socket,
                       context: InboundConnectionContext| async move {
            let protocol = ClientResponseInboundProtocol::new(request.protocol.response_protocol());
            let defaults = recorded_route_defaults();
            let fallback = request.fallback;
            let accept = request
                .protocol
                .accept_recorded_tcp_route(socket)
                .await
                .map_err(EngineError::from)?;
            let accept: Option<PreparedInboundRouteAccept<_, _>> = accept
                .map(|result| prepare_inbound_route_accept(result, fallback))
                .transpose()?;
            context
                .dispatch_recorded_mux_tcp_route(Ok(accept), protocol, defaults)
                .await
        },
        dispatch_quic: |request: PreparedVlessInboundRequest,
                        stream,
                        context: InboundConnectionContext| async move {
            let protocol = ClientResponseInboundProtocol::new(request.protocol.response_protocol());
            let defaults = recorded_route_defaults();
            let fallback = request.fallback;
            let accept = request
                .protocol
                .accept_recorded_stream_route(stream)
                .await
                .map_err(EngineError::from)?;
            let accept = prepare_inbound_route_accept(accept, fallback)?;
            context
                .dispatch_recorded_mux_stream_route(Ok(accept), protocol, defaults)
                .await
        },
    })
}

fn recorded_route_defaults() -> RecordedProtocolMuxRouteDefaults {
    RecordedProtocolMuxRouteDefaults {
        udp_protocol: VlessInboundListenerRequest::UDP_PROTOCOL,
        mux_protocol: VlessInboundListenerRequest::MUX_PROTOCOL,
        panic_message: VlessInboundListenerRequest::PANIC_MESSAGE,
        abort_on_end: VlessInboundListenerRequest::ABORT_ON_END,
        udp_accept_log_message: Some("MUX stream accepted"),
    }
}
