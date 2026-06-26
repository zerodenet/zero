use zero_engine::ResolvedLeafOutbound;

/// `(server, port)` of a carrier leaf, for diagnostics.
pub(super) fn carrier_upstream(
    proxy: &crate::runtime::Proxy,
    leaf: &ResolvedLeafOutbound<'_>,
) -> (String, u16) {
    proxy
        .protocols
        .outbound_leaf_runtime(leaf)
        .ok()
        .and_then(|runtime| runtime.endpoint.map(|endpoint| endpoint.upstream()))
        .unwrap_or_default()
}
