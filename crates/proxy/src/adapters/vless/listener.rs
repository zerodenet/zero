use ::vless::transport::{
    VlessInboundListenerRequest, VlessInboundOptionsRef, VlessInboundUserRef,
    VlessRealityServerOptionsRef, VlessTransportRuntime,
};
use zero_config::{InboundConfig, InboundProtocolConfig};
use zero_engine::EngineError;

use crate::runtime::inbound_operation::{
    InboundConnectionContext, TcpOrQuicInboundListenerOperation,
};
use crate::runtime::inbound_route::RecordedProtocolMuxRouteDefaults;
use crate::runtime::tcp_ingress::ClientResponseInboundProtocol;

pub(super) fn prepare(
    runtime: VlessTransportRuntime,
    inbound: InboundConfig,
    source_dir: Option<&std::path::Path>,
) -> Result<Box<dyn crate::runtime::inbound_operation::PreparedInboundListenerOperation>, EngineError>
{
    let request: VlessInboundListenerRequest = match &inbound.protocol {
        InboundProtocolConfig::Vless {
            users,
            reality,
            tls,
            ws,
            grpc,
            h2,
            http_upgrade,
            split_http,
            fallback,
            ..
        } => runtime
            .build_inbound_listener_request(
                source_dir,
                VlessInboundOptionsRef {
                    users: users.iter().map(|user| VlessInboundUserRef {
                        id: user.id.as_str(),
                        flow: user.flow.as_deref(),
                        credential_id: user.credential_id.as_deref(),
                        principal_key: user.principal_key.as_deref(),
                        up_bps: user.up_bps,
                        down_bps: user.down_bps,
                    }),
                    reality: reality
                        .as_deref()
                        .map(|reality| VlessRealityServerOptionsRef {
                            private_key: reality.private_key.as_str(),
                            short_ids: reality.short_ids.as_slice(),
                            server_name: reality.server_name.as_deref(),
                            cipher_suites: reality.cipher_suites.as_slice(),
                        }),
                    tls: tls.as_deref(),
                    ws: ws.as_deref(),
                    grpc: grpc.as_deref(),
                    h2: h2.as_deref(),
                    http_upgrade: http_upgrade.as_deref(),
                    split_http: split_http.as_deref(),
                    fallback: fallback.as_deref(),
                },
            )
            .map_err(EngineError::from)?,
        _ => {
            return Err(EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "vless inbound listener received non-vless inbound config",
            )));
        }
    };
    Ok(Box::new(TcpOrQuicInboundListenerOperation {
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
    }))
}
