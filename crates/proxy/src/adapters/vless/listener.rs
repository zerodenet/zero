use zero_config::InboundConfig;
use zero_engine::EngineError;
use zero_transport::vless_transport::VlessInboundListenerRequest;

use crate::protocol_registry::BoundInbound;
use crate::runtime::inbound_route::{
    dispatch_recorded_protocol_mux_stream_request_with_defaults,
    dispatch_recorded_protocol_mux_tcp_request_with_defaults, RecordedProtocolMuxRouteDefaults,
};
use crate::runtime::listener_loop::{
    run_logged_quic_stream_listener_loop, run_logged_tcp_socket_listener_loop,
    LoggedQuicStreamListenerRequest, LoggedTcpSocketListenerRequest,
};
use crate::runtime::tcp_ingress::ClientResponseInboundProtocol;

pub(super) fn prepare(
    inbound: InboundConfig,
    source_dir: Option<&std::path::Path>,
) -> Result<Box<dyn crate::runtime::inbound_operation::PreparedInboundListenerOperation>, EngineError>
{
    let request = VlessInboundListenerRequest::from_protocol_config(&inbound.protocol, source_dir)?;
    Ok(Box::new(
        crate::runtime::inbound_operation::InboundListenerOperation::new(
            move |proxy, bound: crate::protocol_registry::BoundInbound, shutdown_rx| async move {
                let protocol_name = request.protocol_name();
                let error_protocol_name = request.error_protocol_name();
                let inbound_tag = inbound.tag;

                match bound {
                    BoundInbound::Tcp(listener) => {
                        run_logged_tcp_socket_listener_loop(LoggedTcpSocketListenerRequest {
                            proxy: &proxy,
                            inbound_tag,
                            protocol_name,
                            error_protocol_name,
                            request,
                            listener,
                            shutdown: shutdown_rx,
                            dispatch: |proxy,
                                       request: VlessInboundListenerRequest,
                                       inbound_tag,
                                       socket,
                                       source_addr| async move {
                                let protocol =
                                    ClientResponseInboundProtocol::new(request.response_protocol());
                                let defaults: RecordedProtocolMuxRouteDefaults =
                                    request.recorded_mux_route_defaults().into();
                                dispatch_recorded_protocol_mux_tcp_request_with_defaults(
                                    request.accept_recorded_tcp_route(socket).await,
                                    proxy,
                                    inbound_tag,
                                    source_addr,
                                    protocol,
                                    defaults,
                                )
                                .await
                            },
                        })
                        .await
                    }
                    BoundInbound::Quic(listener) => {
                        run_logged_quic_stream_listener_loop(LoggedQuicStreamListenerRequest {
                            proxy: &proxy,
                            inbound_tag,
                            protocol_name,
                            error_protocol_name,
                            request,
                            listener,
                            shutdown: shutdown_rx,
                            dispatch: |proxy,
                                       request: VlessInboundListenerRequest,
                                       inbound_tag,
                                       stream| async move {
                                let protocol =
                                    ClientResponseInboundProtocol::new(request.response_protocol());
                                let defaults: RecordedProtocolMuxRouteDefaults =
                                    request.recorded_mux_route_defaults().into();
                                dispatch_recorded_protocol_mux_stream_request_with_defaults(
                                    request.accept_recorded_stream_route(stream).await,
                                    proxy,
                                    inbound_tag,
                                    None,
                                    protocol,
                                    defaults,
                                )
                                .await
                            },
                        })
                        .await
                    }
                }
            },
        ),
    ))
}
