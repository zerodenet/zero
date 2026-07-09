use zero_engine::EngineError;

use crate::protocol_registry::BoundInbound;

pub(in crate::protocol_registry) async fn bind_tcp_inbound(
    inbound: &zero_config::InboundConfig,
) -> Result<BoundInbound, EngineError> {
    let listen = format!("{}:{}", inbound.listen.address, inbound.listen.port);
    let tcp = zero_platform_tokio::TokioListener::bind(&listen)
        .await
        .map_err(EngineError::Io)?;
    Ok(BoundInbound::Tcp(tcp))
}

#[cfg(feature = "transport_quic")]
pub(crate) async fn bind_transport_inbound<P>(
    inbound: &zero_config::InboundConfig,
    source_dir: Option<&std::path::Path>,
) -> Result<BoundInbound, EngineError>
where
    P: zero_transport::inbound_route::ProtocolInboundBindPlan,
{
    let listen = format!("{}:{}", inbound.listen.address, inbound.listen.port);
    let plan = P::from_protocol_config(&inbound.protocol, source_dir)?;
    match plan.bind(&listen).await? {
        zero_transport::inbound_route::TransportInboundBindTarget::Tcp => {
            bind_tcp_inbound(inbound).await
        }
        zero_transport::inbound_route::TransportInboundBindTarget::Quic(endpoint) => {
            Ok(BoundInbound::Quic(endpoint))
        }
    }
}
