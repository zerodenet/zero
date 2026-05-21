mod support;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use zero_api::{
    event_type, ApiErrorCode, CommandRequest, CommandService, ConfigQuery, ConfigValidateCommand,
    EventFilter, EventSource, FlowCloseCommand, FlowFilter, FlowGetQuery, FlowListQuery,
    PoliciesQuery, PolicySelectCommand, QueryRequest, QueryResponse, QueryService,
};
use zero_config::RuntimeConfig;
use zero_proxy::Proxy as Engine;

use serde_json::json;
use support::{free_port, spawn_engine, wait_for, wait_for_listener};

#[tokio::test]
async fn exports_serializable_engine_status_view() {
    let echo_port = free_port();
    let proxy_port = free_port();

    let echo_task = tokio::spawn(async move {
        let listener = TcpListener::bind(("127.0.0.1", echo_port))
            .await
            .expect("bind echo");
        let (mut stream, _) = listener.accept().await.expect("accept echo");
        let mut buf = [0_u8; 4];
        stream.read_exact(&mut buf).await.expect("read echo");
        stream.write_all(&buf).await.expect("write echo");
    });

    let config = RuntimeConfig::parse(&format!(
        r#"{{
            "inbounds": [
                {{
                    "tag": "socks-in",
                    "listen": {{ "address": "127.0.0.1", "port": {proxy_port} }},
                    "protocol": {{ "type": "socks5" }}
                }}
            ],
            "outbounds": [
                {{
                    "tag": "chain",
                    "protocol": {{
                        "type": "socks5",
                        "server": "127.0.0.1",
                        "port": 2080
                    }}
                }}
            ],
            "route": {{
                "rules": [],
                "final": {{ "type": "direct" }}
            }}
        }}"#
    ))
    .expect("parse config");

    let engine = Engine::new(config).expect("build engine");
    let handle = spawn_engine(engine);

    wait_for_listener(proxy_port).await;

    let mut client = TcpStream::connect(("127.0.0.1", proxy_port))
        .await
        .expect("connect proxy");
    client
        .write_all(&[0x05, 0x01, 0x00])
        .await
        .expect("write auth");

    let mut auth = [0_u8; 2];
    client.read_exact(&mut auth).await.expect("read auth");

    let request = [
        0x05,
        0x01,
        0x00,
        0x01,
        127,
        0,
        0,
        1,
        ((echo_port >> 8) & 0xff) as u8,
        (echo_port & 0xff) as u8,
    ];
    client.write_all(&request).await.expect("write request");

    let mut response = [0_u8; 10];
    client
        .read_exact(&mut response)
        .await
        .expect("read response");
    assert_eq!(response[1], 0x00);

    let exported = handle.export_status();
    assert_eq!(exported.config.mode.kind, "rule");
    assert_eq!(exported.config.rule_count, 0);
    assert_eq!(exported.config.inbounds.len(), 1);
    assert_eq!(exported.config.inbounds[0].tag, "socks-in");
    assert_eq!(exported.config.inbounds[0].protocol, "socks5");
    assert_eq!(exported.config.outbounds.len(), 1);
    assert_eq!(exported.config.outbounds[0].tag, "chain");
    assert_eq!(exported.config.outbounds[0].protocol, "socks5");
    assert!(exported.config.outbound_groups.is_empty());
    assert_eq!(exported.runtime.udp_upstream_idle_timeout_seconds, 30);
    assert_eq!(exported.runtime.active_sessions.len(), 1);
    assert_eq!(exported.runtime.active_sessions[0].target.family, "ipv4");
    assert_eq!(exported.runtime.active_sessions[0].protocol, "socks5");
    assert_eq!(exported.runtime.active_sessions[0].network, "tcp");
    assert_eq!(exported.runtime.active_sessions[0].mode, "rule");
    assert!(exported.runtime.recent_completed_sessions.is_empty());

    let json = serde_json::to_value(&exported).expect("serialize export");
    assert_eq!(json["config"]["mode"]["kind"], "rule");
    assert_eq!(json["config"]["inbounds"][0]["tag"], "socks-in");
    assert_eq!(json["config"]["outbounds"][0]["server"], "127.0.0.1");
    assert_eq!(json["runtime"]["udp_upstream_idle_timeout_seconds"], 30);
    assert_eq!(json["runtime"]["active_sessions"][0]["network"], "tcp");
    assert_eq!(json["runtime"]["active_sessions"][0]["mode"], "rule");
    assert_eq!(
        json["runtime"]["active_sessions"][0]["target"]["family"],
        "ipv4"
    );

    client.write_all(b"ping").await.expect("write payload");
    let mut echoed = [0_u8; 4];
    client.read_exact(&mut echoed).await.expect("read payload");
    assert_eq!(&echoed, b"ping");

    drop(client);
    wait_for("completed flow event", || {
        !handle.completed_sessions().is_empty()
    })
    .await;

    let events = handle
        .subscribe(EventFilter::default())
        .expect("subscribe events");
    let completed = events
        .iter()
        .find(|event| event.event_type == event_type::FLOW_COMPLETED)
        .expect("flow completed event");
    assert!(
        completed.sequence.unwrap() >= 1,
        "sequence should be >=1 (engine.started takes seq 1)"
    );
    assert_eq!(completed.payload["flow_id"], "1");
    assert_eq!(completed.payload["network"], "tcp");
    assert_eq!(completed.payload["inbound"]["tag"], "socks-in");
    assert_eq!(completed.payload["outbound"]["protocol"], "direct");
    assert!(completed.payload["traffic"]["bytes_up"].as_u64().unwrap() >= 4);
    assert!(completed.payload["traffic"]["bytes_down"].as_u64().unwrap() >= 4);

    let filtered = handle
        .subscribe(EventFilter {
            inbound_tags: vec!["socks-in".to_owned()],
            ..EventFilter::default()
        })
        .expect("subscribe filtered events");
    assert!(
        filtered.len() >= 1,
        "should have at least 1 flow.completed event"
    );

    handle.shutdown().await.expect("shutdown engine");
    let _ = echo_task.await;
}

#[test]
fn exports_custom_udp_upstream_idle_timeout_from_config() {
    let config = RuntimeConfig::parse(
        r#"{
            "runtime": {
                "udp_upstream_idle_timeout_seconds": 9
            },
            "route": {
                "rules": [],
                "final": { "type": "direct" }
            }
        }"#,
    )
    .expect("parse config");

    let engine = Engine::new(config).expect("build engine");
    let exported = engine.export_runtime();

    assert_eq!(exported.udp_upstream_idle_timeout_seconds, 9);
}

#[test]
fn selector_group_update_is_reflected_in_exported_config() {
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
    .expect("parse config");

    let engine = Engine::new(config).expect("build engine");
    assert_eq!(
        engine.export_config().outbound_groups[0]
            .selected
            .as_deref(),
        Some("block")
    );

    engine
        .set_selector_target("proxy", "direct")
        .expect("update selector");

    assert_eq!(
        engine.export_config().outbound_groups[0]
            .selected
            .as_deref(),
        Some("direct")
    );
}

#[test]
fn selector_group_can_switch_to_group_target_in_exported_config() {
    let config = RuntimeConfig::parse(
        r#"{
            "outbounds": [
                { "tag": "direct", "protocol": { "type": "direct" } },
                { "tag": "block", "protocol": { "type": "block" } }
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
                    "outbounds": ["direct", "fallback-proxy"],
                    "selected": "direct"
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
    .expect("parse config");

    let engine = Engine::new(config).expect("build engine");
    engine
        .set_selector_target("proxy", "fallback-proxy")
        .expect("update selector");

    let exported = engine.export_config();
    let group = exported
        .outbound_groups
        .iter()
        .find(|group| group.tag == "proxy")
        .expect("find selector group");
    assert_eq!(group.selected.as_deref(), Some("fallback-proxy"));
    assert_eq!(
        group.effective_chains,
        vec![
            vec![
                "proxy".to_owned(),
                "fallback-proxy".to_owned(),
                "block".to_owned(),
            ],
            vec![
                "proxy".to_owned(),
                "fallback-proxy".to_owned(),
                "direct".to_owned(),
            ],
        ]
    );
}

#[test]
fn engine_command_service_validates_config_and_selects_policy() {
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
    .expect("parse config");

    let engine = Engine::new(config).expect("build engine");
    let validation = engine
        .execute(CommandRequest::ConfigValidate(ConfigValidateCommand {
            config: json!({
                "route": {
                    "rules": [],
                    "final": { "type": "direct" }
                }
            }),
        }))
        .expect("validate config");
    assert!(validation.accepted);
    assert_eq!(validation.result.expect("result")["valid"], true);

    let selected = engine
        .execute(CommandRequest::PolicySelect(PolicySelectCommand {
            policy_tag: "proxy".to_owned(),
            target_tag: "direct".to_owned(),
        }))
        .expect("select policy");
    assert!(selected.accepted);
    assert_eq!(
        engine.export_config().outbound_groups[0]
            .selected
            .as_deref(),
        Some("direct")
    );

    let missing = engine
        .execute(CommandRequest::PolicySelect(PolicySelectCommand {
            policy_tag: "missing".to_owned(),
            target_tag: "direct".to_owned(),
        }))
        .expect_err("missing policy should fail");
    assert_eq!(missing.code, ApiErrorCode::NotFound);

    let not_found = engine
        .execute(CommandRequest::FlowClose(FlowCloseCommand {
            flow_id: "999999".to_owned(),
        }))
        .expect_err("closing non-existent flow should fail");
    assert_eq!(not_found.code, ApiErrorCode::NotFound);
}

#[test]
fn engine_query_service_exposes_config_and_policy_snapshots() {
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
    .expect("parse config");

    let engine = Engine::new(config).expect("build engine");

    let config = engine
        .query(QueryRequest::Config(ConfigQuery))
        .expect("query config");
    let QueryResponse::Config(config) = config else {
        panic!("expected config query response");
    };
    assert_eq!(config.value["mode"]["kind"], "global");
    assert_eq!(config.value["mode"]["outbound"], "proxy");

    let policies = engine
        .query(QueryRequest::Policies(PoliciesQuery))
        .expect("query policies");
    let QueryResponse::Policies(policies) = policies else {
        panic!("expected policies query response");
    };
    assert_eq!(policies.value.as_array().expect("policies array").len(), 1);
    assert_eq!(policies.value[0]["tag"], "proxy");
    assert_eq!(policies.value[0]["selected"], "block");
}

#[test]
fn engine_query_service_reports_flow_lookup_and_filter_boundaries() {
    let config = RuntimeConfig::parse(
        r#"{
            "route": {
                "rules": [],
                "final": { "type": "direct" }
            }
        }"#,
    )
    .expect("parse config");

    let engine = Engine::new(config).expect("build engine");

    let missing = engine
        .query(QueryRequest::Flow(FlowGetQuery {
            flow_id: "missing".to_owned(),
        }))
        .expect_err("missing flow should fail");
    assert_eq!(missing.code, ApiErrorCode::NotFound);

    let filtered = engine
        .query(QueryRequest::ActiveFlows(FlowListQuery {
            limit: None,
            filter: FlowFilter {
                principal_key: Some("user:10001".to_owned()),
                ..FlowFilter::default()
            },
        }))
        .expect("principal filter should be supported");
    let QueryResponse::Flows(filtered) = filtered else {
        panic!("expected flows query response");
    };
    let flows = filtered.value.as_array().expect("flows array");
    assert!(flows.is_empty());
}
