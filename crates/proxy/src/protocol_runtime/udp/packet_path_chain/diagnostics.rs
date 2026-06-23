use zero_engine::ResolvedLeafOutbound;

/// `(server, port)` of a carrier leaf, for diagnostics.
pub(super) fn carrier_upstream(leaf: &ResolvedLeafOutbound<'_>) -> (String, u16) {
    crate::runtime::orchestration::endpoint(leaf)
        .map(|endpoint| (endpoint.server.to_owned(), endpoint.port))
        .unwrap_or_default()
}
