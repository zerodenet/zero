use zero_config::{InboundConfig, InboundProtocolConfig};
use zero_engine::EngineError;
use zero_transport::inbound_stack::build_required_tls_acceptor;

use ::trojan::transport::TrojanInboundListenerRequest;

use crate::runtime::inbound_operation::TcpInboundListenerOperation;

pub(super) fn prepare(
    inbound: InboundConfig,
    source_dir: Option<&std::path::Path>,
) -> Result<Box<dyn crate::runtime::inbound_operation::PreparedInboundListenerOperation>, EngineError>
{
    let request = match &inbound.protocol {
        InboundProtocolConfig::Trojan { password, tls, .. } => {
            let profile = ::trojan::inbound::TrojanInboundProfile::from_config_password(password);
            let tls_acceptor =
                build_required_tls_acceptor(source_dir, tls.as_ref(), "trojan requires TLS")?;
            TrojanInboundListenerRequest::new(profile, tls_acceptor)
        }
        _ => {
            return Err(EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "trojan inbound listener received non-trojan inbound config",
            )));
        }
    };
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
