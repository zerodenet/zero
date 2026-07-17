use zero_engine::EngineError;

use crate::protocol_registry::BoundInbound;

pub(crate) async fn bind_tcp_inbound(
    inbound: &zero_config::InboundConfig,
) -> Result<BoundInbound, EngineError> {
    let listen = format!("{}:{}", inbound.listen.address, inbound.listen.port);
    let tcp = zero_platform_tokio::TokioListener::bind(&listen)
        .await
        .map_err(EngineError::Io)?;
    Ok(BoundInbound::Tcp(tcp))
}

pub(crate) fn inbound_listen_addr(inbound: &zero_config::InboundConfig) -> String {
    format!("{}:{}", inbound.listen.address, inbound.listen.port)
}
