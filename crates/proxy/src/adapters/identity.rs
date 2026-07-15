//! Adapter identity, support predicates, and transport-bridge classification.

use zero_config::{InboundProtocolConfig, OutboundProtocolConfig};

#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
use crate::runtime::path::TcpPathCategory;

pub(crate) trait NamedProtocolAdapter {
    const PROTOCOL_NAME: &'static str;
    const FEATURE_NAME: &'static str;
    const HAS_INBOUND: bool = true;
    const HAS_OUTBOUND: bool = true;
}

#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
pub(crate) trait ProtocolTransportBridgeAdapter: NamedProtocolAdapter {
    type Bridge;

    const TCP_PATH: TcpPathCategory;
}

pub(crate) fn named_protocol_supports_inbound<A>(config: &InboundProtocolConfig) -> bool
where
    A: NamedProtocolAdapter,
{
    A::HAS_INBOUND && config.protocol_name() == A::PROTOCOL_NAME
}

pub(crate) fn named_protocol_supports_outbound<A>(config: &OutboundProtocolConfig) -> bool
where
    A: NamedProtocolAdapter,
{
    A::HAS_OUTBOUND && config.protocol_name() == A::PROTOCOL_NAME
}
