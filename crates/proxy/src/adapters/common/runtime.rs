use zero_engine::{EngineError, ResolvedLeafOutbound};

use super::errors::{tcp_failure, udp_flow_failure};
use super::named::ProtocolTransportBridgeAdapter;
use crate::protocol_registry::OutboundLeafRuntime;
use crate::runtime::orchestration::{OutboundEndpoint, TcpPathCategory};
use crate::runtime::udp_dispatch::FlowFailure;
use crate::transport::TcpOutboundFailure;

pub(crate) fn protocol_leaf_runtime<'a>(
    tag: &'a str,
    server: &'a str,
    port: u16,
    tcp_path: TcpPathCategory,
) -> OutboundLeafRuntime<'a> {
    OutboundLeafRuntime {
        tcp_path,
        health_tag: Some(tag),
        endpoint: Some(OutboundEndpoint { server, port }),
        kernel_tag: None,
        udp_policy_tag: Some(tag),
    }
}

pub(crate) fn transport_bridge_adapter_leaf_runtime<'a, A>(
    leaf: &ResolvedLeafOutbound<'a>,
) -> Option<OutboundLeafRuntime<'a>>
where
    A: ProtocolTransportBridgeAdapter,
{
    proxy_leaf_runtime(leaf, A::TCP_PATH)
}

pub(crate) fn direct_leaf_runtime<'a>(
    leaf: &ResolvedLeafOutbound<'a>,
) -> Option<OutboundLeafRuntime<'a>> {
    match leaf {
        ResolvedLeafOutbound::Direct { tag } => Some(OutboundLeafRuntime {
            tcp_path: TcpPathCategory::Direct,
            health_tag: None,
            endpoint: None,
            kernel_tag: *tag,
            udp_policy_tag: *tag,
        }),
        _ => None,
    }
}

pub(crate) fn proxy_leaf_runtime<'a>(
    leaf: &ResolvedLeafOutbound<'a>,
    tcp_path: TcpPathCategory,
) -> Option<OutboundLeafRuntime<'a>> {
    let tag = leaf.tag()?;
    let (server, port) = leaf.proxy_endpoint()?;

    Some(protocol_leaf_runtime(tag, server, port, tcp_path))
}

/// Build a `TcpOutboundFailure` for the impossible case where an adapter's
/// `connect_tcp` receives a leaf variant it did not claim.
///
/// `claims_outbound_leaf` guarantees the variant matches before the runtime
/// dispatches `connect_tcp`, so this only fires on a programming error.
pub(crate) fn unreachable_leaf(
    adapter: &'static str,
    _leaf: &ResolvedLeafOutbound<'_>,
) -> TcpOutboundFailure {
    tcp_failure(
        "outbound_leaf_mismatch",
        EngineError::Io(std::io::Error::other(format!(
            "{adapter} adapter received a non-matching outbound leaf"
        ))),
        None,
    )
}

/// Same as [`unreachable_leaf`] but for the UDP `start_udp_flow` path.
pub(crate) fn unreachable_udp_leaf(
    adapter: &'static str,
    _leaf: &ResolvedLeafOutbound<'_>,
) -> FlowFailure {
    udp_flow_failure(
        "udp_leaf_mismatch",
        EngineError::Io(std::io::Error::other(format!(
            "{adapter} adapter received a non-matching UDP leaf"
        ))),
        None,
    )
}
