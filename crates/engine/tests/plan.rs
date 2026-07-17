use std::time::Duration;

use zero_config::RuntimeConfig;
use zero_engine::{Engine, EnginePlan, TargetId, TargetKind};

#[test]
fn builds_engine_plan_for_nested_groups() {
    let config = RuntimeConfig::parse(
        r#"{
            "outbounds": [
                { "tag": "direct", "protocol": { "type": "direct" } },
                { "tag": "block", "protocol": { "type": "block" } },
                {
                    "tag": "chain-a",
                    "protocol": {
                        "type": "socks5",
                        "server": "127.0.0.1",
                        "port": 2080
                    }
                }
            ],
            "outbound_groups": [
                {
                    "tag": "fallback-proxy",
                    "type": "fallback",
                    "outbounds": ["chain-a", "direct"]
                },
                {
                    "tag": "proxy",
                    "type": "selector",
                    "outbounds": ["fallback-proxy", "direct"],
                    "selected": "fallback-proxy"
                },
                {
                    "tag": "probe",
                    "type": "url_test",
                    "outbounds": ["proxy", "direct"],
                    "url": "http://example.com/",
                    "interval_seconds": 30
                }
            ],
            "mode": {
                "type": "global",
                "outbound": "probe"
            },
            "route": {
                "rules": [],
                "final": { "type": "direct" }
            }
        }"#,
    )
    .expect("parse config");

    let plan = EnginePlan::build(&config).expect("build engine plan");
    let direct_id = plan.target_id("direct").expect("find direct target");
    let chain_a_id = plan.target_id("chain-a").expect("find chain-a target");
    let fallback_id = plan
        .target_id("fallback-proxy")
        .expect("find fallback target");
    let selector_id = plan.target_id("proxy").expect("find selector target");
    let urltest_id = plan.target_id("probe").expect("find urltest target");

    let direct = plan.target(direct_id).expect("resolve direct target");
    assert_eq!(direct.tag(), "direct");
    assert!(matches!(
        direct.kind(),
        TargetKind::Outbound(outbound)
            if outbound.runtime_kind() == zero_config::OutboundRuntimeKind::Direct
    ));

    let fallback = plan.target(fallback_id).expect("resolve fallback target");
    let TargetKind::Fallback(fallback_group) = fallback.kind() else {
        panic!("fallback-proxy should compile as a fallback group");
    };
    assert_eq!(
        fallback_group
            .members()
            .iter()
            .map(|member| plan_tag(&plan, *member))
            .collect::<Vec<_>>(),
        vec!["chain-a", "direct"]
    );

    let selector = plan.target(selector_id).expect("resolve selector target");
    let TargetKind::Selector(selector_group) = selector.kind() else {
        panic!("proxy should compile as a selector group");
    };
    assert_eq!(
        selector_group
            .members()
            .iter()
            .map(|member| plan_tag(&plan, *member))
            .collect::<Vec<_>>(),
        vec!["fallback-proxy", "direct"]
    );
    assert_eq!(
        plan_tag(&plan, selector_group.initial_member()),
        "fallback-proxy"
    );

    let urltest = plan.target(urltest_id).expect("resolve urltest target");
    let TargetKind::UrlTest(urltest_group) = urltest.kind() else {
        panic!("probe should compile as a urltest group");
    };
    assert_eq!(
        urltest_group
            .members()
            .iter()
            .map(|member| plan_tag(&plan, *member))
            .collect::<Vec<_>>(),
        vec!["proxy", "direct"]
    );
    assert_eq!(plan_tag(&plan, urltest_group.initial_member()), "proxy");
    assert_eq!(urltest_group.url(), "http://example.com/");
    assert_eq!(urltest_group.interval(), Duration::from_secs(30));

    assert_eq!(plan.selector_groups(), &[selector_id]);
    assert_eq!(plan.urltest_groups(), &[urltest_id]);
    assert_eq!(plan_tag(&plan, chain_a_id), "chain-a");
}

#[test]
fn engine_exposes_compiled_plan() {
    let config = RuntimeConfig::parse(
        r#"{
            "outbounds": [
                { "tag": "direct", "protocol": { "type": "direct" } },
                { "tag": "block", "protocol": { "type": "block" } }
            ],
            "outbound_groups": [
                {
                    "tag": "proxy",
                    "type": "selector",
                    "outbounds": ["block", "direct"],
                    "selected": "block"
                }
            ],
            "route": {
                "rules": [],
                "final": { "type": "route", "outbound": "proxy" }
            }
        }"#,
    )
    .expect("parse config");

    let engine = Engine::new(config).expect("build engine");
    let plan = engine.plan();
    let selector_id = plan.target_id("proxy").expect("find selector target");
    let selector = plan.target(selector_id).expect("resolve selector target");

    let TargetKind::Selector(selector_group) = selector.kind() else {
        panic!("proxy should compile as a selector group");
    };
    assert_eq!(plan_tag(&plan, selector_group.initial_member()), "block");
    assert_eq!(plan.selector_groups(), &[selector_id]);
}

#[test]
fn builds_engine_plan_for_loadbalance_group() {
    let config = RuntimeConfig::parse(
        r#"{
            "outbounds": [
                { "tag": "direct", "protocol": { "type": "direct" } },
                { "tag": "block", "protocol": { "type": "block" } }
            ],
            "outbound_groups": [
                {
                    "tag": "lb",
                    "type": "load_balance",
                    "outbounds": ["direct", "block"],
                    "strategy": "round_robin"
                }
            ],
            "mode": { "type": "global", "outbound": "lb" },
            "route": { "rules": [], "final": { "type": "direct" } }
        }"#,
    )
    .expect("parse config");

    let plan = EnginePlan::build(&config).expect("build engine plan");
    let lb_id = plan.target_id("lb").expect("find loadbalance target");
    let lb = plan.target(lb_id).expect("resolve loadbalance target");
    let TargetKind::LoadBalance(lb_group) = lb.kind() else {
        panic!("lb should compile as a loadbalance group");
    };
    assert_eq!(lb_group.members().len(), 2);
    assert_eq!(plan_tag(&plan, lb_group.initial_member()), "direct");
    assert_eq!(plan.loadbalance_groups(), &[lb_id]);
}

#[test]
fn mieru_outbound_defaults_username_to_password_in_plan() {
    let config = RuntimeConfig::parse(
        r#"{
            "outbounds": [
                {
                    "tag": "mieru-node",
                    "protocol": {
                        "type": "mieru",
                        "server": "example.com",
                        "port": 2999,
                        "password": "318149df-2bab-4a35-9de1-870f3e410598"
                    }
                }
            ],
            "route": {
                "rules": [],
                "final": { "type": "route", "outbound": "mieru-node" }
            }
        }"#,
    )
    .expect("parse config");

    let plan = EnginePlan::build(&config).expect("build engine plan");
    let target_id = plan.target_id("mieru-node").expect("find mieru target");
    let target = plan.target(target_id).expect("resolve mieru target");
    let TargetKind::Outbound(outbound) = target.kind() else {
        panic!("mieru-node should compile as an outbound");
    };
    assert_eq!(outbound.protocol(), "mieru");
    let zero_config::OutboundProtocolConfig::Mieru {
        username, password, ..
    } = &config.outbounds[0].protocol
    else {
        panic!("expected normalized mieru config");
    };
    let username = username.as_deref().expect("normalized username");
    assert_eq!(username, password);
    assert_eq!(username, "318149df-2bab-4a35-9de1-870f3e410598");
}

fn plan_tag(plan: &EnginePlan, target_id: TargetId) -> &str {
    plan.target(target_id).expect("resolve target").tag()
}
