use ::vmess::transport::{
    VmessInboundListenerRequest, VmessInboundOptionsRef, VmessInboundUserRef, VmessTransportRuntime,
};
use zero_config::{InboundConfig, InboundProtocolConfig};
use zero_engine::EngineError;

use crate::runtime::inbound_operation::TcpInboundListenerOperation;
use crate::runtime::inbound_route::NoClientMuxRouteDefaults;

pub(super) fn prepare(
    runtime: VmessTransportRuntime,
    inbound: InboundConfig,
    source_dir: Option<&std::path::Path>,
) -> Result<Box<dyn crate::runtime::inbound_operation::PreparedInboundListenerOperation>, EngineError>
{
    let request: VmessInboundListenerRequest = match &inbound.protocol {
        InboundProtocolConfig::Vmess {
            users,
            tls,
            ws,
            grpc,
        } => runtime
            .build_inbound_listener_request(
                source_dir,
                VmessInboundOptionsRef {
                    users: users.iter().map(|user| VmessInboundUserRef {
                        id: user.id.as_str(),
                        cipher: user.cipher.as_str(),
                        credential_id: user.credential_id.as_deref(),
                        principal_key: user.principal_key.as_deref(),
                        up_bps: user.up_bps,
                        down_bps: user.down_bps,
                    }),
                    tls: tls.as_deref(),
                    ws: ws.as_deref(),
                    grpc: grpc.as_deref(),
                },
            )
            .map_err(EngineError::from)?,
        _ => {
            return Err(EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "vmess inbound listener received non-vmess inbound config",
            )));
        }
    };
    Ok(Box::new(TcpInboundListenerOperation {
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
    }))
}
