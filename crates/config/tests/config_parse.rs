use zero_config::{
    InboundProtocolConfig, OutboundProtocolConfig, RouteActionConfig, RuleConditionConfig,
    RuntimeConfig,
};

#[test]
fn parses_config_into_adts() {
    let config = RuntimeConfig::parse(
        r#"{
            "inbounds": [
                {
                    "tag": "socks-in",
                    "listen": { "address": "127.0.0.1", "port": 1080 },
                    "protocol": { "type": "socks5" }
                },
                {
                    "tag": "http-in",
                    "listen": { "address": "127.0.0.1", "port": 8080 },
                    "protocol": { "type": "http-connect" }
                }
            ],
            "outbounds": [
                {
                    "tag": "direct",
                    "protocol": { "type": "direct" }
                },
                {
                    "tag": "block",
                    "protocol": { "type": "block" }
                },
                {
                    "tag": "chain",
                    "protocol": { "type": "socks5", "server": "127.0.0.1", "port": 2080 }
                }
            ],
            "route": {
                "rules": [
                    {
                        "condition": {
                            "type": "or",
                            "items": [
                                { "type": "domain", "values": ["blocked.example"] },
                                { "type": "ip", "values": ["10.0.0.0/8"] }
                            ]
                        },
                        "action": { "type": "route", "outbound": "block" }
                    }
                ],
                "final": { "type": "direct" }
            }
        }"#,
    )
    .expect("config should parse");

    assert!(matches!(
        config.inbounds[0].protocol,
        InboundProtocolConfig::Socks5
    ));
    assert!(matches!(
        config.inbounds[1].protocol,
        InboundProtocolConfig::HttpConnect
    ));
    assert!(matches!(
        config.outbounds[0].protocol,
        OutboundProtocolConfig::Direct
    ));
    assert!(matches!(
        config.outbounds[1].protocol,
        OutboundProtocolConfig::Block
    ));
    assert!(matches!(
        config.outbounds[2].protocol,
        OutboundProtocolConfig::Socks5 { .. }
    ));
    assert!(matches!(
        config.route.final_action,
        RouteActionConfig::Direct
    ));
    assert!(matches!(
        config.route.rules[0].condition,
        RuleConditionConfig::Or { .. }
    ));
}

#[test]
fn rejects_undefined_outbound_reference() {
    let error = RuntimeConfig::parse(
        r#"{
            "outbounds": [],
            "route": {
                "rules": [],
                "final": { "type": "route", "outbound": "missing" }
            }
        }"#,
    )
    .expect_err("config should fail");

    assert!(matches!(
        error,
        zero_config::ConfigError::UndefinedOutboundTag { .. }
    ));
}

#[test]
fn accepts_http_alias_and_block_action_alias() {
    let config = RuntimeConfig::parse(
        r#"{
            "inbounds": [
                {
                    "tag": "http-in",
                    "listen": { "address": "127.0.0.1", "port": 8080 },
                    "protocol": { "type": "http" }
                }
            ],
            "route": {
                "rules": [],
                "final": { "type": "block" }
            }
        }"#,
    )
    .expect("config should parse");

    assert!(matches!(
        config.inbounds[0].protocol,
        InboundProtocolConfig::HttpConnect
    ));
    assert!(matches!(
        config.route.final_action,
        RouteActionConfig::Reject
    ));
}

#[test]
fn accepts_mixed_inbound_type() {
    let config = RuntimeConfig::parse(
        r#"{
            "inbounds": [
                {
                    "tag": "mixed-in",
                    "listen": { "address": "127.0.0.1", "port": 1080 },
                    "protocol": { "type": "mixed" }
                }
            ],
            "route": {
                "rules": [],
                "final": { "type": "direct" }
            }
        }"#,
    )
    .expect("config should parse");

    assert!(matches!(
        config.inbounds[0].protocol,
        InboundProtocolConfig::Mixed
    ));
}

#[test]
fn rejects_duplicate_inbound_listen_endpoint() {
    let error = RuntimeConfig::parse(
        r#"{
            "inbounds": [
                {
                    "tag": "socks-in",
                    "listen": { "address": "127.0.0.1", "port": 1080 },
                    "protocol": { "type": "socks5" }
                },
                {
                    "tag": "http-in",
                    "listen": { "address": "127.0.0.1", "port": 1080 },
                    "protocol": { "type": "http-connect" }
                }
            ],
            "route": {
                "rules": [],
                "final": { "type": "direct" }
            }
        }"#,
    )
    .expect_err("config should fail");

    assert!(matches!(
        error,
        zero_config::ConfigError::DuplicateInboundListen { .. }
    ));
}

#[test]
fn parses_utf8_bom_prefixed_json() {
    let config = RuntimeConfig::parse(
        "\u{feff}{\n  \"inbounds\": [],\n  \"route\": { \"rules\": [], \"final\": { \"type\": \"direct\" } }\n}",
    )
    .expect("config with utf-8 bom should parse");

    assert!(config.inbounds.is_empty());
    assert!(matches!(
        config.route.final_action,
        RouteActionConfig::Direct
    ));
}
