use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use zero_config::RuntimeConfig;
use zero_engine::Engine;

#[test]
fn engine_builds_router_from_config() {
    let config = RuntimeConfig::parse(
        r#"{
            "inbounds": [],
            "outbounds": [],
            "route": {
                "rules": [
                    {
                        "condition": {
                            "type": "domain",
                            "values": ["blocked.example"]
                        },
                        "action": { "type": "reject" }
                    }
                ],
                "final": { "type": "direct" }
            }
        }"#,
    )
    .expect("config should parse");

    let engine = Engine::new(config).expect("engine should build");
    let action = engine.route_for(&zero_core::Address::Domain("blocked.example".to_owned()));

    assert!(matches!(action, zero_router::RouteAction::Reject));
}

#[test]
fn direct_mode_overrides_router_decision() {
    let config = RuntimeConfig::parse(
        r#"{
            "inbounds": [],
            "outbounds": [],
            "mode": { "type": "direct" },
            "route": {
                "rules": [
                    {
                        "condition": {
                            "type": "domain",
                            "values": ["blocked.example"]
                        },
                        "action": { "type": "reject" }
                    }
                ],
                "final": { "type": "direct" }
            }
        }"#,
    )
    .expect("config should parse");

    let engine = Engine::new(config).expect("engine should build");
    let action = engine.route_for(&zero_core::Address::Domain("blocked.example".to_owned()));

    assert!(matches!(action, zero_router::RouteAction::Direct));
}

#[test]
fn global_mode_routes_through_selector_group_tag() {
    let config = RuntimeConfig::parse(
        r#"{
            "inbounds": [],
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
                "final": { "type": "reject" }
            }
        }"#,
    )
    .expect("config should parse");

    let engine = Engine::new(config).expect("engine should build");
    let action = engine.route_for(&zero_core::Address::Domain("example.com".to_owned()));

    assert!(matches!(
        action,
        zero_router::RouteAction::Route(ref tag) if tag == "proxy"
    ));
}

#[test]
fn rule_set_routes_domain_and_cidr_targets() {
    let project_dir = temp_test_dir("engine-rule-set-router");
    let domain_rules = project_dir.join("ads.txt");
    let cidr_rules = project_dir.join("lan.txt");
    fs::write(&domain_rules, "blocked.example\n.ads.local\n").expect("write domain rules");
    fs::write(&cidr_rules, "10.0.0.0/8\n").expect("write cidr rules");

    let config = RuntimeConfig::parse(&format!(
        r#"{{
            "outbounds": [
                {{
                    "tag": "direct",
                    "protocol": {{ "type": "direct" }}
                }},
                {{
                    "tag": "block",
                    "protocol": {{ "type": "block" }}
                }}
            ],
            "route": {{
                "rule_sets": [
                    {{
                        "tag": "ads",
                        "type": "file",
                        "path": "{}",
                        "format": "domain_list"
                    }},
                    {{
                        "tag": "lan",
                        "type": "file",
                        "path": "{}",
                        "format": "cidr_list"
                    }}
                ],
                "rules": [
                    {{
                        "condition": {{ "type": "rule_set", "tag": "ads" }},
                        "action": {{ "type": "route", "outbound": "block" }}
                    }},
                    {{
                        "condition": {{ "type": "rule_set", "tag": "lan" }},
                        "action": {{ "type": "route", "outbound": "direct" }}
                    }}
                ],
                "final": {{ "type": "direct" }}
            }}
        }}"#,
        escape_json_path(&domain_rules),
        escape_json_path(&cidr_rules),
    ))
    .expect("config should parse");

    let engine = Engine::new(config).expect("engine should build");

    let domain_action = engine.route_for(&zero_core::Address::Domain("api.ads.local".to_owned()));
    assert!(matches!(domain_action, zero_router::RouteAction::Route(ref tag) if tag == "block"));

    let ip_action = engine.route_for(&zero_core::Address::Ipv4([10, 1, 2, 3]));
    assert!(matches!(ip_action, zero_router::RouteAction::Route(ref tag) if tag == "direct"));

    cleanup_temp_dir(&project_dir);
}

#[test]
fn route_rules_can_match_inbound_tag_to_group() {
    let config = RuntimeConfig::parse(
        r#"{
            "inbounds": [
                {
                    "tag": "hk-in",
                    "listen": { "address": "127.0.0.1", "port": 7891 },
                    "protocol": { "type": "mixed" }
                },
                {
                    "tag": "jp-in",
                    "listen": { "address": "127.0.0.1", "port": 7892 },
                    "protocol": { "type": "mixed" }
                }
            ],
            "outbounds": [
                { "tag": "hk-a", "protocol": { "type": "direct" } },
                { "tag": "hk-b", "protocol": { "type": "direct" } },
                { "tag": "jp-a", "protocol": { "type": "direct" } },
                { "tag": "jp-b", "protocol": { "type": "direct" } }
            ],
            "outbound_groups": [
                {
                    "tag": "hk-lb",
                    "type": "load_balance",
                    "outbounds": ["hk-a", "hk-b"],
                    "strategy": "round_robin"
                },
                {
                    "tag": "jp-lb",
                    "type": "load_balance",
                    "outbounds": ["jp-a", "jp-b"],
                    "strategy": "round_robin"
                }
            ],
            "route": {
                "rules": [
                    {
                        "condition": { "type": "inbound", "values": ["hk-in"] },
                        "action": { "type": "route", "outbound": "hk-lb" }
                    },
                    {
                        "condition": { "type": "inbound", "values": ["jp-in"] },
                        "action": { "type": "route", "outbound": "jp-lb" }
                    }
                ],
                "final": { "type": "direct" }
            }
        }"#,
    )
    .expect("config should parse");

    let engine = Engine::new(config).expect("engine should build");
    let address = zero_core::Address::Domain("example.com".to_owned());

    let hk_action = engine.route_decision_with_inbound(&address, None, Some("hk-in"));
    assert!(matches!(
        hk_action,
        zero_engine::RouteDecision::Route(ref tag) if tag == "hk-lb"
    ));

    let jp_action = engine.route_decision_with_inbound(&address, None, Some("jp-in"));
    assert!(matches!(
        jp_action,
        zero_engine::RouteDecision::Route(ref tag) if tag == "jp-lb"
    ));

    let default_action = engine.route_decision_with_inbound(&address, None, None);
    assert!(matches!(default_action, zero_engine::RouteDecision::Direct));
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

fn escape_json_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "\\\\")
}
