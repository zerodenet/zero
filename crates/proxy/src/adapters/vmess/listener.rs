use zero_config::InboundConfig;
use zero_engine::EngineError;
use zero_transport::vmess_transport::VmessInboundListenerRequest;

use crate::runtime::inbound_operation::TcpInboundListenerOperation;
use crate::runtime::inbound_route::NoClientMuxRouteDefaults;

pub(super) fn prepare(
    inbound: InboundConfig,
    source_dir: Option<&std::path::Path>,
) -> Result<Box<dyn crate::runtime::inbound_operation::PreparedInboundListenerOperation>, EngineError>
{
    let request = VmessInboundListenerRequest::from_protocol_config(&inbound.protocol, source_dir)?;
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
