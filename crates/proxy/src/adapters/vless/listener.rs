use ::vless::transport::VlessInboundListenerRequest;
use zero_config::{InboundConfig, InboundProtocolConfig};
use zero_engine::EngineError;

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
    let request = match &inbound.protocol {
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
        } => {
            let profile =
                vless::inbound::VlessInboundProfile::from_config_users(users.iter().map(|user| {
                    (
                        user.id.as_str(),
                        user.flow.as_deref(),
                        user.credential_id.as_deref(),
                        user.principal_key.as_deref(),
                        user.up_bps,
                        user.down_bps,
                    )
                }))
                .map_err(EngineError::from)?;
            let reality = reality.as_deref().map(|reality| {
                vless::reality::VlessRealityServerProfile::from_config_server(
                    reality.private_key.clone(),
                    reality.short_ids.clone(),
                    reality.server_name.clone(),
                    reality.cipher_suites.clone(),
                )
            });
            VlessInboundListenerRequest::from_profiles(
                source_dir,
                profile,
                reality,
                tls.as_deref(),
                ws.as_deref(),
                grpc.as_deref(),
                h2.as_deref(),
                http_upgrade.as_deref(),
                split_http.as_deref(),
                fallback.as_deref(),
            )?
        }
        _ => {
            return Err(EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "vless inbound listener received non-vless inbound config",
            )));
        }
    };
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
