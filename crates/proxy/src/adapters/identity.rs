//! Adapter identity, support predicates, and transport-bridge classification.

use zero_config::{InboundProtocolConfig, OutboundProtocolConfig};

pub(crate) trait NamedProtocolAdapter {
    const PROTOCOL_NAME: &'static str;
    const FEATURE_NAME: &'static str;
    const HAS_INBOUND: bool = true;
    const HAS_OUTBOUND: bool = true;
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
