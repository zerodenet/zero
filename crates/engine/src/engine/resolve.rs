use std::sync::Arc;

use zero_config::{OutboundConfig, OutboundGroupConfig, OutboundGroupKind, OutboundProtocolConfig};

use super::outbound_group_state::OutboundGroupStateStore;

pub(crate) enum ResolvedLeafOutbound<'a> {
    Direct {
        tag: Option<&'a str>,
    },
    Block {
        tag: Option<&'a str>,
    },
    Socks5 {
        tag: &'a str,
        server: &'a str,
        port: u16,
    },
}

pub(crate) enum ResolvedOutbound<'a> {
    Single(ResolvedLeafOutbound<'a>),
    Fallback {
        candidates: Vec<ResolvedLeafOutbound<'a>>,
    },
}

pub(crate) fn resolve_named_outbound(outbound: &OutboundConfig) -> ResolvedLeafOutbound<'_> {
    match outbound.protocol {
        OutboundProtocolConfig::Direct => ResolvedLeafOutbound::Direct {
            tag: Some(outbound.tag()),
        },
        OutboundProtocolConfig::Block => ResolvedLeafOutbound::Block {
            tag: Some(outbound.tag()),
        },
        OutboundProtocolConfig::Socks5 { ref server, port } => ResolvedLeafOutbound::Socks5 {
            tag: outbound.tag(),
            server,
            port,
        },
    }
}

pub(crate) fn resolve_group<'a>(
    group: &'a OutboundGroupConfig,
    outbounds: &'a [OutboundConfig],
    outbound_group_state: &Arc<OutboundGroupStateStore>,
) -> Option<ResolvedOutbound<'a>> {
    match &group.group {
        OutboundGroupKind::Selector { .. } => {
            let selected = outbound_group_state
                .selector_selected_outbound(group.tag())
                .or_else(|| group.active_outbound().map(str::to_owned))?;
            outbounds
                .iter()
                .find(|outbound| outbound.tag() == selected)
                .map(resolve_named_outbound)
                .map(ResolvedOutbound::Single)
        }
        OutboundGroupKind::Fallback { outbounds: members } => {
            let mut candidates = Vec::with_capacity(members.len());
            for member in members {
                let outbound = outbounds.iter().find(|outbound| outbound.tag() == member)?;
                candidates.push(resolve_named_outbound(outbound));
            }

            Some(ResolvedOutbound::Fallback { candidates })
        }
        OutboundGroupKind::UrlTest { .. } => {
            let selected = outbound_group_state
                .selected_outbound(group.tag())
                .or_else(|| group.active_outbound().map(str::to_owned))?;
            outbounds
                .iter()
                .find(|outbound| outbound.tag() == selected)
                .map(resolve_named_outbound)
                .map(ResolvedOutbound::Single)
        }
    }
}
