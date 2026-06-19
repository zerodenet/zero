use super::{EngineError, FlowFailure, ResolvedLeafOutbound, TcpOutboundFailure};

/// Build a `TcpOutboundFailure` for the impossible case where an adapter's
/// `connect_tcp` receives a leaf variant it did not claim.
///
/// `claims_outbound_leaf` guarantees the variant matches before the runtime
/// dispatches `connect_tcp`, so this only fires on a programming error.
pub(super) fn unreachable_leaf(
    adapter: &'static str,
    _leaf: &ResolvedLeafOutbound<'_>,
) -> TcpOutboundFailure {
    TcpOutboundFailure {
        stage: "outbound_leaf_mismatch",
        error: EngineError::Io(std::io::Error::other(format!(
            "{adapter} adapter received a non-matching outbound leaf"
        ))),
        upstream_endpoint: None,
    }
}

/// Same as [`unreachable_leaf`] but for the UDP `start_udp_flow` path.
pub(super) fn unreachable_udp_leaf(
    adapter: &'static str,
    _leaf: &ResolvedLeafOutbound<'_>,
) -> FlowFailure {
    FlowFailure {
        stage: "udp_leaf_mismatch",
        error: EngineError::Io(std::io::Error::other(format!(
            "{adapter} adapter received a non-matching UDP leaf"
        ))),
        upstream: None,
    }
}
