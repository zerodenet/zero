use super::*;

fn inbound_protocol_name(config: &InboundProtocolConfig) -> &'static str {
    match config {
        InboundProtocolConfig::Socks5 { .. } => "socks5",
        InboundProtocolConfig::HttpConnect => "http_connect",
        InboundProtocolConfig::Mixed { .. } => "mixed",
        InboundProtocolConfig::Vless { .. } => "vless",
        InboundProtocolConfig::Hysteria2 { .. } => "hysteria2",
        InboundProtocolConfig::Shadowsocks { .. } => "shadowsocks",
        InboundProtocolConfig::Trojan { .. } => "trojan",
        InboundProtocolConfig::Vmess { .. } => "vmess",
        InboundProtocolConfig::Direct { .. } => "direct",
        InboundProtocolConfig::Mieru { .. } => "mieru",
    }
}

fn outbound_leaf_name(leaf: &ResolvedLeafOutbound<'_>) -> &'static str {
    match leaf {
        ResolvedLeafOutbound::Direct { .. } => "direct",
        ResolvedLeafOutbound::Block { .. } => "block",
        ResolvedLeafOutbound::Socks5 { .. } => "socks5",
        ResolvedLeafOutbound::Vless { .. } => "vless",
        ResolvedLeafOutbound::Hysteria2 { .. } => "hysteria2",
        ResolvedLeafOutbound::Shadowsocks { .. } => "shadowsocks",
        ResolvedLeafOutbound::Trojan { .. } => "trojan",
        ResolvedLeafOutbound::Vmess { .. } => "vmess",
        ResolvedLeafOutbound::Mieru { .. } => "mieru",
    }
}

fn compiled_in_inbound_configs() -> Vec<InboundProtocolConfig> {
    let mut configs = vec![InboundProtocolConfig::Direct {
        target: None,
        port: None,
    }];

    #[cfg(feature = "socks5")]
    configs.push(InboundProtocolConfig::Socks5 { users: Vec::new() });
    #[cfg(feature = "http_connect")]
    configs.push(InboundProtocolConfig::HttpConnect);
    #[cfg(feature = "mixed")]
    configs.push(InboundProtocolConfig::Mixed {
        socks5_users: Vec::new(),
    });
    #[cfg(feature = "vless")]
    configs.push(InboundProtocolConfig::Vless {
        users: Vec::new(),
        tls: None,
        reality: None,
        ws: None,
        grpc: None,
        h2: None,
        http_upgrade: None,
        fallback: None,
        quic: None,
        split_http: None,
    });
    #[cfg(feature = "hysteria2")]
    configs.push(InboundProtocolConfig::Hysteria2 {
        password: "password".to_string(),
        cert_path: None,
        key_path: None,
        up_bps: None,
        down_bps: None,
    });
    #[cfg(feature = "shadowsocks")]
    configs.push(InboundProtocolConfig::Shadowsocks {
        password: "password".to_string(),
        cipher: "chacha20-ietf-poly1305".to_string(),
        up_bps: None,
        down_bps: None,
    });
    #[cfg(feature = "trojan")]
    configs.push(InboundProtocolConfig::Trojan {
        password: "password".to_string(),
        sni: None,
        tls: None,
        up_bps: None,
        down_bps: None,
    });
    #[cfg(feature = "vmess")]
    configs.push(InboundProtocolConfig::Vmess {
        users: Vec::new(),
        tls: None,
        ws: None,
        grpc: None,
    });
    #[cfg(feature = "mieru")]
    configs.push(InboundProtocolConfig::Mieru { users: Vec::new() });

    configs
}

fn compiled_in_outbound_leaves<'a>() -> Vec<(ResolvedLeafOutbound<'a>, usize)> {
    let mut leaves = vec![
        (
            ResolvedLeafOutbound::Direct {
                tag: Some("direct"),
            },
            1,
        ),
        (ResolvedLeafOutbound::Block { tag: Some("block") }, 0),
    ];

    #[cfg(feature = "socks5")]
    leaves.push((
        ResolvedLeafOutbound::Socks5 {
            tag: "socks5",
            server: "127.0.0.1",
            port: 1080,
            username: None,
            password: None,
        },
        1,
    ));
    #[cfg(feature = "vless")]
    leaves.push((
        ResolvedLeafOutbound::Vless {
            tag: "vless",
            server: "127.0.0.1",
            port: 443,
            id: "00000000-0000-0000-0000-000000000000",
            flow: None,
            mux_concurrency: None,
            mux_idle_timeout_secs: None,
            tls: None,
            reality: None,
            ws: None,
            grpc: None,
            h2: None,
            http_upgrade: None,
            split_http: None,
            quic: None,
        },
        1,
    ));
    #[cfg(feature = "hysteria2")]
    leaves.push((
        ResolvedLeafOutbound::Hysteria2 {
            tag: "hysteria2",
            server: "127.0.0.1",
            port: 443,
            password: "password",
            insecure: false,
            client_fingerprint: None,
        },
        1,
    ));
    #[cfg(feature = "shadowsocks")]
    leaves.push((
        ResolvedLeafOutbound::Shadowsocks {
            tag: "shadowsocks",
            server: "127.0.0.1",
            port: 8388,
            password: "password",
            cipher: "chacha20-ietf-poly1305",
        },
        1,
    ));
    #[cfg(feature = "trojan")]
    leaves.push((
        ResolvedLeafOutbound::Trojan {
            tag: "trojan",
            server: "127.0.0.1",
            port: 443,
            password: "password",
            sni: None,
            insecure: false,
            client_fingerprint: None,
        },
        1,
    ));
    #[cfg(feature = "vmess")]
    leaves.push((
        ResolvedLeafOutbound::Vmess {
            tag: "vmess",
            server: "127.0.0.1",
            port: 443,
            id: "00000000-0000-0000-0000-000000000000",
            cipher: "aes-128-gcm",
            mux_concurrency: None,
            mux_idle_timeout_secs: None,
            tls: None,
            ws: None,
            grpc: None,
        },
        1,
    ));
    #[cfg(feature = "mieru")]
    leaves.push((
        ResolvedLeafOutbound::Mieru {
            tag: "mieru",
            server: "127.0.0.1",
            port: 8964,
            username: "",
            password: "password",
        },
        1,
    ));

    leaves
}

#[test]
fn compiled_in_inbound_variants_have_exactly_one_registered_adapter() {
    let registry = crate::protocol_adapter::ProtocolRegistry::build();

    for config in compiled_in_inbound_configs() {
        let claim_count = registry
            .adapters
            .iter()
            .filter(|adapter| adapter.supports_inbound(&config))
            .count();
        assert_eq!(
            claim_count,
            1,
            "{} inbound config should be claimed by exactly one adapter",
            inbound_protocol_name(&config)
        );
        assert!(
            registry.find_inbound(&config).is_ok(),
            "{} inbound config should resolve through ProtocolRegistry::find_inbound",
            inbound_protocol_name(&config)
        );
    }
}

#[test]
fn compiled_in_outbound_leaf_variants_have_expected_adapter_claims() {
    let registry = crate::protocol_adapter::ProtocolRegistry::build();

    for (leaf, expected_claims) in compiled_in_outbound_leaves() {
        let claim_count = registry
            .adapters
            .iter()
            .filter(|adapter| adapter.claims_outbound_leaf(&leaf))
            .count();
        assert_eq!(
            claim_count,
            expected_claims,
            "{} outbound leaf should have {expected_claims} adapter claim(s)",
            outbound_leaf_name(&leaf)
        );

        let resolved = registry.find_outbound_leaf(&leaf);
        assert_eq!(
            resolved.is_ok(),
            expected_claims == 1,
            "{} outbound leaf registry lookup result did not match claim policy",
            outbound_leaf_name(&leaf)
        );
    }
}
