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
