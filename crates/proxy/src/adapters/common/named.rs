use zero_config::{InboundProtocolConfig, OutboundProtocolConfig};
use zero_engine::ResolvedLeafOutbound;

use crate::runtime::orchestration::TcpPathCategory;

pub(crate) trait NamedProtocolAdapter {
    const PROTOCOL_NAME: &'static str;
    const FEATURE_NAME: &'static str;
    const HAS_INBOUND: bool = true;
    const HAS_OUTBOUND: bool = true;
    const CLAIMS_RUNTIME_LEAF: bool = true;
}

pub(crate) trait ProtocolTransportBridgeAdapter: NamedProtocolAdapter {
    type Bridge;

    const TCP_PATH: TcpPathCategory;

    fn bridge(&self) -> &Self::Bridge;
}

pub(crate) fn named_protocol_claims_runtime_leaf<A>(leaf: &ResolvedLeafOutbound<'_>) -> bool
where
    A: NamedProtocolAdapter,
{
    A::CLAIMS_RUNTIME_LEAF && leaf.protocol_name() == A::PROTOCOL_NAME
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
