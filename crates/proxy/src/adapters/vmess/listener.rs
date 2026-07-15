use ::vmess::transport::{OwnedVmessInboundListenerConfig, VmessInboundListenerRequest};
use zero_config::{InboundConfig, InboundProtocolConfig};
use zero_engine::EngineError;

use crate::runtime::inbound_operation::TcpInboundListenerOperation;
use crate::runtime::inbound_route::NoClientMuxRouteDefaults;

pub(super) fn prepare(
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
        } => {
            let profile = ::vmess::inbound::VmessInboundProfile::from_config_users(
                users.iter().map(|user| {
                    (
                        user.id.as_str(),
                        user.cipher.as_str(),
                        user.credential_id.as_deref(),
                        user.principal_key.as_deref(),
                        user.up_bps,
                        user.down_bps,
                    )
                }),
            )
            .map_err(|error| {
                EngineError::Io(std::io::Error::new(std::io::ErrorKind::InvalidInput, error))
            })?;
            OwnedVmessInboundListenerConfig::from_config_refs(
                source_dir,
                profile,
                tls.as_deref(),
                ws.as_deref(),
                grpc.as_deref(),
            )?
            .into()
        }
        _ => {
            return Err(EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "vmess inbound listener received non-vmess inbound config",
            )));
        }
    };
    Ok(Box::new(TcpInboundListenerOperation {
        inbound_tag: inbound.tag,
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
