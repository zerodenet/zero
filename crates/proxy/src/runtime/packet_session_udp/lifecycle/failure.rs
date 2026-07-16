use tracing::warn;
use zero_engine::EngineError;

use crate::runtime::packet_session_udp::contract::{
    PacketSessionUdpFailurePolicy, PacketSessionUdpHandler,
};

pub(super) async fn handle_runtime_failure<H>(
    handler: &mut H,
    failure_policy: PacketSessionUdpFailurePolicy,
    inbound_tag: &str,
    protocol: &'static str,
    message: &'static str,
    error: EngineError,
) -> Result<(), EngineError>
where
    H: PacketSessionUdpHandler,
{
    match failure_policy {
        PacketSessionUdpFailurePolicy::ReturnError => Err(error),
        #[cfg(any(feature = "vless", feature = "vmess"))]
        PacketSessionUdpFailurePolicy::LogAndBreak => {
            warn!(
                inbound_tag = inbound_tag,
                protocol = protocol,
                error = %error,
                "{message}"
            );
            let _ = handler.finish().await;
            Ok(())
        }
    }
}
