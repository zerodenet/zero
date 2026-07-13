use zero_config::InboundConfig;
use zero_engine::EngineError;
use zero_transport::trojan_transport::TrojanInboundListenerRequest;

use crate::runtime::inbound_operation::TcpInboundListenerOperation;

pub(super) fn prepare(
    inbound: InboundConfig,
    source_dir: Option<&std::path::Path>,
) -> Result<Box<dyn crate::runtime::inbound_operation::PreparedInboundListenerOperation>, EngineError>
{
    let request =
        TrojanInboundListenerRequest::from_protocol_config(&inbound.protocol, source_dir)?;
    Ok(Box::new(TcpInboundListenerOperation {
        inbound_tag: inbound.tag,
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
    }))
}
