use zero_engine::{EngineError, ResolvedLeafOutbound};

use crate::protocol_registry::OutboundLeafRuntime;
use crate::runtime::orchestration::{OutboundEndpoint, TcpPathCategory};
use crate::runtime::udp_dispatch::FlowFailure;
use crate::transport::TcpOutboundFailure;

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

pub(super) fn direct_leaf_runtime<'a>(
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

pub(super) fn proxy_leaf_runtime<'a>(
    leaf: &ResolvedLeafOutbound<'a>,
    tcp_path: TcpPathCategory,
) -> Option<OutboundLeafRuntime<'a>> {
    let (tag, server, port) = match leaf {
        ResolvedLeafOutbound::Socks5 {
            tag, server, port, ..
        }
        | ResolvedLeafOutbound::Vless {
            tag, server, port, ..
        }
        | ResolvedLeafOutbound::Hysteria2 {
            tag, server, port, ..
        }
        | ResolvedLeafOutbound::Shadowsocks {
            tag, server, port, ..
        }
        | ResolvedLeafOutbound::Trojan {
            tag, server, port, ..
        }
        | ResolvedLeafOutbound::Vmess {
            tag, server, port, ..
        }
        | ResolvedLeafOutbound::Mieru {
            tag, server, port, ..
        } => (*tag, *server, *port),
        _ => return None,
    };

    Some(OutboundLeafRuntime {
        tcp_path,
        health_tag: Some(tag),
        endpoint: Some(OutboundEndpoint { server, port }),
        kernel_tag: None,
        udp_policy_tag: Some(tag),
    })
}
