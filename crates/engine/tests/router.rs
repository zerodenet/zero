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
