use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use zero_config::{
    EventSinkConfig, InboundProtocolConfig, ModeConfig, OutboundGroupKind, OutboundProtocolConfig,
    RouteActionConfig, RuleConditionConfig, RuntimeConfig,
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
            "outbound_groups": [
                {
                    "tag": "proxy",
                    "type": "selector",
                    "outbounds": ["chain", "direct"],
                    "selected": "chain"
                }
            ],
            "runtime": {
                "udp_upstream_idle_timeout_seconds": 12
            },
            "mode": {
                "type": "global",
                "outbound": "proxy"
            },
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
        InboundProtocolConfig::Socks5 { .. }
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
        config.outbound_groups[0].group,
        OutboundGroupKind::Selector { .. }
    ));
    assert_eq!(config.runtime.udp_upstream_idle_timeout_seconds, 12);
    assert!(matches!(config.mode, ModeConfig::Global { .. }));
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
fn parses_api_event_sinks_and_control_config() {
    let config = RuntimeConfig::parse(
        r#"{
            "api": {
                "event_sinks": [
                    {
                        "tag": "panel",
                        "type": "webhook",
                        "url": "https://panel.example.com/api/zero/events",
                        "events": ["flow.completed", "engine.warning"],
                        "source_id": "edge-01",
                        "api_key_env": "ZERO_PANEL_API_KEY"
                    },
                    {
                        "tag": "local-events",
                        "type": "jsonl",
                        "path": "zero-events.jsonl",
                        "events": ["flow.completed"]
                    }
                ],
                "control": {
                    "enabled": true,
                    "listen": { "address": "127.0.0.1", "port": 9090 },
                    "api_key_env": "ZERO_NODE_API_KEY"
                }
            },
            "route": {
                "rules": [],
                "final": { "type": "direct" }
            }
        }"#,
    )
    .expect("config should parse");

    assert_eq!(config.api.event_sinks.len(), 2);
    let EventSinkConfig::Webhook {
        tag,
        url,
        events,
        source_id,
        api_key_env,
        ..
    } = &config.api.event_sinks[0]
    else {
        panic!("expected webhook sink");
    };
    assert_eq!(tag, "panel");
    assert_eq!(url, "https://panel.example.com/api/zero/events");
    assert_eq!(events, &["flow.completed", "engine.warning"]);
    assert_eq!(source_id.as_deref(), Some("edge-01"));
    assert_eq!(api_key_env.as_deref(), Some("ZERO_PANEL_API_KEY"));

    assert!(config.api.control.enabled);
    assert_eq!(
        config.api.control.listen.as_ref().expect("listen").port,
        9090
    );
}

#[test]
fn rejects_unknown_api_event_type() {
    let error = RuntimeConfig::parse(
        r#"{
            "api": {
                "event_sinks": [
                    {
                        "tag": "panel",
                        "type": "webhook",
                        "url": "https://panel.example.com/api/zero/events",
                        "events": ["panel.user.changed"],
                        "api_key": "secret"
                    }
                ]
            },
            "route": {
                "rules": [],
                "final": { "type": "direct" }
            }
        }"#,
    )
    .expect_err("unknown event type should fail");

    assert!(matches!(error, zero_config::ConfigError::InvalidApi(_)));
}

#[test]
fn rejects_insecure_webhook_without_explicit_opt_in() {
    let error = RuntimeConfig::parse(
        r#"{
            "api": {
                "event_sinks": [
                    {
                        "tag": "panel",
                        "type": "webhook",
                        "url": "http://127.0.0.1:9000/events",
                        "events": ["flow.completed"],
                        "api_key": "secret"
                    }
                ]
            },
            "route": {
                "rules": [],
                "final": { "type": "direct" }
            }
        }"#,
    )
    .expect_err("http webhook should require allow_insecure");

    assert!(matches!(error, zero_config::ConfigError::InvalidApi(_)));
}

#[test]
fn runtime_idle_timeout_defaults_to_thirty_seconds() {
    let config = RuntimeConfig::parse(
        r#"{
            "inbounds": [],
            "route": {
                "rules": [],
                "final": { "type": "direct" }
            }
        }"#,
    )
    .expect("config should parse");

    assert_eq!(config.runtime.udp_upstream_idle_timeout_seconds, 30);
}

#[test]
fn rejects_zero_udp_upstream_idle_timeout() {
    let error = RuntimeConfig::parse(
        r#"{
            "runtime": {
                "udp_upstream_idle_timeout_seconds": 0
            },
            "route": {
                "rules": [],
                "final": { "type": "direct" }
            }
        }"#,
    )
    .expect_err("config should fail");

    assert!(matches!(error, zero_config::ConfigError::InvalidRuntime(_)));
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
        zero_config::ConfigError::UndefinedRouteTargetTag { .. }
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
        InboundProtocolConfig::Mixed { .. }
    ));
}

#[test]
fn parses_socks5_inbound_and_outbound_auth() {
    let config = RuntimeConfig::parse(
        r#"{
            "inbounds": [
                {
                    "tag": "socks-in",
                    "listen": { "address": "127.0.0.1", "port": 1080 },
                    "protocol": {
                        "type": "socks5",
                        "users": [
                            { "username": "alice", "password": "secret" }
                        ]
                    }
                },
                {
                    "tag": "mixed-in",
                    "listen": { "address": "127.0.0.1", "port": 1081 },
                    "protocol": {
                        "type": "mixed",
                        "socks5_users": [
                            { "username": "bob", "password": "secret" }
                        ]
                    }
                }
            ],
            "outbounds": [
                {
                    "tag": "chain",
                    "protocol": {
                        "type": "socks5",
                        "server": "127.0.0.1",
                        "port": 2080,
                        "username": "upstream",
                        "password": "secret"
                    }
                }
            ],
            "route": {
                "rules": [],
                "final": { "type": "route", "outbound": "chain" }
            }
        }"#,
    )
    .expect("config should parse");

    assert_eq!(
        config.inbounds[0].protocol.socks5_users()[0].username,
        "alice"
    );
    assert_eq!(
        config.inbounds[1].protocol.socks5_users()[0].username,
        "bob"
    );
    match &config.outbounds[0].protocol {
        OutboundProtocolConfig::Socks5 {
            username, password, ..
        } => {
            assert_eq!(username.as_deref(), Some("upstream"));
            assert_eq!(password.as_deref(), Some("secret"));
        }
        _ => panic!("expected socks5 outbound"),
    }
}

#[test]
fn rejects_partial_socks5_outbound_auth() {
    let error = RuntimeConfig::parse(
        r#"{
            "outbounds": [
                {
                    "tag": "chain",
                    "protocol": {
                        "type": "socks5",
                        "server": "127.0.0.1",
                        "port": 2080,
                        "username": "upstream"
                    }
                }
            ],
            "route": {
                "rules": [],
                "final": { "type": "route", "outbound": "chain" }
            }
        }"#,
    )
    .expect_err("config should fail");

    assert!(matches!(
        error,
        zero_config::ConfigError::InvalidOutbound(_)
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

#[test]
fn selector_group_requires_defined_member_outbounds() {
    let error = RuntimeConfig::parse(
        r#"{
            "outbounds": [
                {
                    "tag": "direct",
                    "protocol": { "type": "direct" }
                }
            ],
            "outbound_groups": [
                {
                    "tag": "proxy",
                    "type": "selector",
                    "outbounds": ["missing"]
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
        zero_config::ConfigError::InvalidOutboundGroup(_)
    ));
}

#[test]
fn global_mode_accepts_selector_group_target() {
    let config = RuntimeConfig::parse(
        r#"{
            "outbounds": [
                {
                    "tag": "direct",
                    "protocol": { "type": "direct" }
                }
            ],
            "outbound_groups": [
                {
                    "tag": "proxy",
                    "type": "selector",
                    "outbounds": ["direct"]
                }
            ],
            "mode": {
                "type": "global",
                "outbound": "proxy"
            },
            "route": {
                "rules": [],
                "final": { "type": "direct" }
            }
        }"#,
    )
    .expect("config should parse");

    assert!(matches!(config.mode, ModeConfig::Global { .. }));
}

#[test]
fn accepts_fallback_group_type() {
    let config = RuntimeConfig::parse(
        r#"{
            "outbounds": [
                {
                    "tag": "direct",
                    "protocol": { "type": "direct" }
                },
                {
                    "tag": "chain",
                    "protocol": { "type": "socks5", "server": "127.0.0.1", "port": 2080 }
                }
            ],
            "outbound_groups": [
                {
                    "tag": "proxy",
                    "type": "fallback",
                    "outbounds": ["chain", "direct"]
                }
            ],
            "mode": {
                "type": "global",
                "outbound": "proxy"
            },
            "route": {
                "rules": [],
                "final": { "type": "direct" }
            }
        }"#,
    )
    .expect("config should parse");

    assert!(matches!(
        config.outbound_groups[0].group,
        OutboundGroupKind::Fallback { .. }
    ));
}

#[test]
fn accepts_urltest_group_type() {
    let config = RuntimeConfig::parse(
        r#"{
            "outbounds": [
                {
                    "tag": "direct",
                    "protocol": { "type": "direct" }
                },
                {
                    "tag": "chain",
                    "protocol": { "type": "socks5", "server": "127.0.0.1", "port": 2080 }
                }
            ],
            "outbound_groups": [
                {
                    "tag": "proxy",
                    "type": "urltest",
                    "outbounds": ["chain", "direct"],
                    "url": "http://127.0.0.1:8081/",
                    "interval_seconds": 15
                }
            ],
            "mode": {
                "type": "global",
                "outbound": "proxy"
            },
            "route": {
                "rules": [],
                "final": { "type": "direct" }
            }
        }"#,
    )
    .expect("config should parse");

    assert!(matches!(
        config.outbound_groups[0].group,
        OutboundGroupKind::UrlTest { .. }
    ));
}

#[test]
fn accepts_group_member_referencing_another_group() {
    let config = RuntimeConfig::parse(
        r#"{
            "outbounds": [
                {
                    "tag": "direct",
                    "protocol": { "type": "direct" }
                },
                {
                    "tag": "block",
                    "protocol": { "type": "block" }
                }
            ],
            "outbound_groups": [
                {
                    "tag": "fallback-proxy",
                    "type": "fallback",
                    "outbounds": ["block", "direct"]
                },
                {
                    "tag": "proxy",
                    "type": "selector",
                    "outbounds": ["fallback-proxy", "direct"],
                    "selected": "fallback-proxy"
                }
            ],
            "mode": {
                "type": "global",
                "outbound": "proxy"
            },
            "route": {
                "rules": [],
                "final": { "type": "reject" }
            }
        }"#,
    )
    .expect("config should parse");

    assert_eq!(config.outbound_groups.len(), 2);
    assert!(matches!(
        config.outbound_groups[0].group,
        OutboundGroupKind::Fallback { .. }
    ));
    assert!(matches!(
        config.outbound_groups[1].group,
        OutboundGroupKind::Selector { .. }
    ));
}

#[test]
fn rejects_group_reference_cycle() {
    let error = RuntimeConfig::parse(
        r#"{
            "outbounds": [
                {
                    "tag": "direct",
                    "protocol": { "type": "direct" }
                }
            ],
            "outbound_groups": [
                {
                    "tag": "group-a",
                    "type": "selector",
                    "outbounds": ["group-b"],
                    "selected": "group-b"
                },
                {
                    "tag": "group-b",
                    "type": "fallback",
                    "outbounds": ["group-a"]
                }
            ],
            "mode": {
                "type": "global",
                "outbound": "group-a"
            },
            "route": {
                "rules": [],
                "final": { "type": "reject" }
            }
        }"#,
    )
    .expect_err("config should fail");

    assert!(matches!(
        error,
        zero_config::ConfigError::InvalidOutboundGroup(_)
    ));
}

#[test]
fn loads_rule_set_from_relative_file_path() {
    let project_dir = temp_test_dir("config-rule-set-relative");
    let rules_dir = project_dir.join("rules");
    fs::create_dir_all(&rules_dir).expect("create rules dir");
    fs::write(rules_dir.join("ads.txt"), "blocked.example\n.ads.local\n").expect("write rules");

    let config_path = project_dir.join("config.json");
    fs::write(
        &config_path,
        r#"{
            "outbounds": [
                { "tag": "block", "protocol": { "type": "block" } }
            ],
            "route": {
                "rule_sets": [
                    {
                        "tag": "ads",
                        "type": "file",
                        "path": "rules/ads.txt",
                        "format": "domain-list"
                    }
                ],
                "rules": [
                    {
                        "condition": { "type": "rule-set", "tag": "ads" },
                        "action": { "type": "route", "outbound": "block" }
                    }
                ],
                "final": { "type": "direct" }
            }
        }"#,
    )
    .expect("write config");

    let config = RuntimeConfig::load_from_path(&config_path).expect("load config");

    assert_eq!(config.source_dir(), Some(project_dir.as_path()));
    assert!(matches!(
        config.route.rules[0].condition,
        RuleConditionConfig::RuleSet { .. }
    ));

    cleanup_temp_dir(&project_dir);
}

#[test]
fn rejects_undefined_rule_set_reference() {
    let error = RuntimeConfig::parse(
        r#"{
            "route": {
                "rules": [
                    {
                        "condition": { "type": "rule-set", "tag": "ads" },
                        "action": { "type": "direct" }
                    }
                ],
                "final": { "type": "direct" }
            }
        }"#,
    )
    .expect_err("config should fail");

    assert!(matches!(
        error,
        zero_config::ConfigError::UndefinedRuleSetTag { .. }
    ));
}

#[test]
fn rejects_invalid_cidr_rule_set_entry() {
    let project_dir = temp_test_dir("config-rule-set-invalid-cidr");
    let rules_dir = project_dir.join("rules");
    fs::create_dir_all(&rules_dir).expect("create rules dir");
    fs::write(rules_dir.join("lan.txt"), "10.0.0.0/8\nnot-a-cidr\n").expect("write rules");

    let config_path = project_dir.join("config.json");
    fs::write(
        &config_path,
        r#"{
            "route": {
                "rule_sets": [
                    {
                        "tag": "lan",
                        "type": "file",
                        "path": "rules/lan.txt",
                        "format": "cidr-list"
                    }
                ],
                "rules": [
                    {
                        "condition": { "type": "rule-set", "tag": "lan" },
                        "action": { "type": "direct" }
                    }
                ],
                "final": { "type": "direct" }
            }
        }"#,
    )
    .expect("write config");

    let error = RuntimeConfig::load_from_path(&config_path).expect_err("config should fail");
    assert!(matches!(error, zero_config::ConfigError::InvalidRuleSet(_)));

    cleanup_temp_dir(&project_dir);
}

fn temp_test_dir(prefix: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after epoch")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("{prefix}-{nonce}"));
    fs::create_dir_all(&dir).expect("create temp test dir");
    dir
}

fn cleanup_temp_dir(path: &Path) {
    let _ = fs::remove_dir_all(path);
}
