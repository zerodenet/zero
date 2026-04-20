use zero_config::{OutboundConfig, OutboundProtocolConfig};

pub(crate) enum ResolvedOutbound<'a> {
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

pub(crate) fn resolve_named_outbound(outbound: &OutboundConfig) -> ResolvedOutbound<'_> {
    match outbound.protocol {
        OutboundProtocolConfig::Direct => ResolvedOutbound::Direct {
            tag: Some(outbound.tag()),
        },
        OutboundProtocolConfig::Block => ResolvedOutbound::Block {
            tag: Some(outbound.tag()),
        },
        OutboundProtocolConfig::Socks5 { ref server, port } => ResolvedOutbound::Socks5 {
            tag: outbound.tag(),
            server,
            port,
        },
    }
}
