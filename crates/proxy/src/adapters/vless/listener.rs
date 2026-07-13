use zero_config::InboundConfig;
use zero_engine::EngineError;
use zero_transport::vless_transport::VlessInboundListenerRequest;

use crate::runtime::inbound_operation::{
    InboundConnectionContext, TcpOrQuicInboundListenerOperation,
};
use crate::runtime::inbound_route::RecordedProtocolMuxRouteDefaults;
use crate::runtime::tcp_ingress::ClientResponseInboundProtocol;

pub(super) fn prepare(
    inbound: InboundConfig,
    source_dir: Option<&std::path::Path>,
) -> Result<Box<dyn crate::runtime::inbound_operation::PreparedInboundListenerOperation>, EngineError>
{
    let request = VlessInboundListenerRequest::from_protocol_config(&inbound.protocol, source_dir)?;
    Ok(Box::new(TcpOrQuicInboundListenerOperation {
        inbound_tag: inbound.tag,
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
                    request.accept_recorded_tcp_route(socket).await,
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
                    request.accept_recorded_stream_route(stream).await,
                    protocol,
                    defaults,
                )
                .await
        },
    }))
}
