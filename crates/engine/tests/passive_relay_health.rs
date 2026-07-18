use zero_config::RuntimeConfig;
use zero_core::Address;
use zero_engine::{
    Engine, OutboundIdentity, PassiveRelayOutcome, ResolvedLeafOutbound, ResolvedOutbound,
    RouteDecision,
};

fn engine() -> Engine {
    let config: RuntimeConfig = serde_json::from_str(
        r#"
        {
          "inbounds": [{
            "tag": "entry",
            "listen": { "address": "127.0.0.1", "port": 10080 },
            "protocol": { "type": "direct", "target": "landing.example", "port": 14788 }
          }],
          "outbounds": [
            {
              "tag": "primary",
              "protocol": {
                "type": "shadowsocks",
                "server": "primary.example",
                "port": 443,
                "password": "password",
                "cipher": "aes-256-gcm"
              }
            },
            {
              "tag": "alternate",
              "protocol": {
                "type": "shadowsocks",
                "server": "alternate.example",
                "port": 443,
                "password": "password",
                "cipher": "aes-256-gcm"
              }
            }
          ],
          "outbound_groups": [{
            "tag": "auto",
            "type": "url_test",
            "outbounds": ["primary", "alternate"],
            "url": "http://probe.example/",
            "interval_seconds": 60
          }],
          "mode": { "type": "global", "outbound": "auto" },
          "route": {
            "rules": [],
            "final": { "type": "route", "outbound": "auto" }
          }
        }
        "#,
    )
    .expect("parse config");
    Engine::new(config).expect("build engine")
}

fn selected_identity(engine: &Engine, target: &Address, port: u16) -> (OutboundIdentity, String) {
    let (resolved, _, selections) = engine
        .resolve_route_decision_for_flow(RouteDecision::Route("auto".to_owned()), target, port)
        .expect("resolve flow");
    let ResolvedOutbound::Single(ResolvedLeafOutbound::Proxy { identity }) = resolved else {
        panic!("expected one proxy leaf");
    };
    (identity, selections[0].member_tag.clone())
}

#[test]
fn early_failures_move_only_the_affected_target_to_an_alternate() {
    let engine = engine();
    let target = Address::Domain("landing.example".to_owned());
    let (primary_identity, primary_tag) = selected_identity(&engine, &target, 14788);
    assert_eq!(primary_identity.config_index(), 0);
    assert_eq!(primary_tag, "primary");

    let selection = zero_engine::PassiveRelaySelection {
        policy_tag: "auto".to_owned(),
        member_tag: "primary".to_owned(),
        half_open: false,
    };
    for _ in 0..3 {
        engine.record_passive_relay_outcome(
            &selection,
            &target,
            14788,
            PassiveRelayOutcome::Failure,
        );
    }

    let (alternate_identity, alternate_tag) = selected_identity(&engine, &target, 14788);
    assert_eq!(alternate_identity.config_index(), 1);
    assert_eq!(alternate_tag, "alternate");

    let (unaffected_identity, unaffected_tag) = selected_identity(&engine, &target, 14688);
    assert_eq!(unaffected_identity.config_index(), 0);
    assert_eq!(unaffected_tag, "primary");
}
