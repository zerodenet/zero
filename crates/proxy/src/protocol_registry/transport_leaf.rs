use std::path::Path;

use zero_engine::ResolvedLeafOutbound;
use zero_transport::outbound_leaf::{PreparedTransportBridgeLeaf, ProtocolTransportLeaf};

pub(crate) enum ResolveTransportLeafError<E> {
    InvalidConfig(E),
    MissingLeaf,
}

pub(crate) trait ProtocolTransportLeafResolver<'a> {
    type TransportLeaf: ProtocolTransportLeaf + 'a;
    type ResolveError: std::fmt::Display;

    fn resolve_transport_leaf(
        &self,
        source_dir: Option<&Path>,
        leaf: &ResolvedLeafOutbound<'a>,
    ) -> Result<Option<Self::TransportLeaf>, Self::ResolveError>;
}

pub(crate) fn prepare_transport_bridge_leaf<'a, TBridge>(
    bridge: &TBridge,
    source_dir: Option<&Path>,
    leaf: &ResolvedLeafOutbound<'a>,
) -> Result<
    PreparedTransportBridgeLeaf<<TBridge as ProtocolTransportLeafResolver<'a>>::TransportLeaf>,
    ResolveTransportLeafError<<TBridge as ProtocolTransportLeafResolver<'a>>::ResolveError>,
>
where
    TBridge: ProtocolTransportLeafResolver<'a>,
{
    bridge
        .resolve_transport_leaf(source_dir, leaf)
        .map_err(ResolveTransportLeafError::InvalidConfig)?
        .map(PreparedTransportBridgeLeaf::new)
        .ok_or(ResolveTransportLeafError::MissingLeaf)
}

pub(crate) fn prepare_last_transport_bridge_leaf<'a, TBridge>(
    bridge: &TBridge,
    chain: &[ResolvedLeafOutbound<'a>],
    source_dir: Option<&Path>,
) -> Result<
    PreparedTransportBridgeLeaf<<TBridge as ProtocolTransportLeafResolver<'a>>::TransportLeaf>,
    ResolveTransportLeafError<<TBridge as ProtocolTransportLeafResolver<'a>>::ResolveError>,
>
where
    TBridge: ProtocolTransportLeafResolver<'a>,
{
    let leaf = chain.last().ok_or(ResolveTransportLeafError::MissingLeaf)?;
    prepare_transport_bridge_leaf(bridge, source_dir, leaf)
}
