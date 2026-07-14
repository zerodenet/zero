use zero_api::{
    event_type, EventFilter, EventSource, PolicyProbeCompletedPayload, PolicyProbeMember,
};
use zero_config::RuntimeConfig;
use zero_engine::{Engine, EngineHandle};

#[test]
fn streams_policy_probe_events_from_the_engine_event_log() {
    let config = RuntimeConfig::parse(
        r#"{
            "inbounds": [],
            "outbounds": [{ "tag": "direct", "protocol": { "type": "direct" } }],
            "route": { "rules": [], "final": { "type": "direct" } }
        }"#,
    )
    .expect("parse config");
    let engine = Engine::new(config).expect("build engine");
    let handle = EngineHandle::new(engine.clone());
    let subscriber = handle
        .subscribe(EventFilter {
            event_types: vec![event_type::POLICY_PROBE_COMPLETED.to_owned()],
            ..EventFilter::default()
        })
        .expect("subscribe to policy probes");

    engine.push_policy_probe_completed(
        "auto",
        PolicyProbeCompletedPayload {
            policy_tag: "auto".to_owned(),
            trigger: "manual".to_owned(),
            url: "http://example.com/".to_owned(),
            started_at_unix_ms: 100,
            completed_at_unix_ms: 125,
            duration_ms: 25,
            selected: Some("direct".to_owned()),
            members: vec![PolicyProbeMember {
                target_tag: "direct".to_owned(),
                healthy: true,
                latency_ms: Some(25),
                error: None,
            }],
        },
    );

    let event = subscriber
        .try_recv()
        .expect("receive live policy probe event");
    assert_eq!(event.event_type, event_type::POLICY_PROBE_COMPLETED);
    assert_eq!(event.payload["trigger"], "manual");
    assert_eq!(event.payload["started_at_unix_ms"], 100);
    assert_eq!(event.payload["completed_at_unix_ms"], 125);
    assert_eq!(event.payload["duration_ms"], 25);
    assert_eq!(event.payload["selected"], "direct");
    assert!(event.sequence.is_some());

    let latest = handle
        .latest(
            1,
            EventFilter {
                event_types: vec![event_type::POLICY_PROBE_COMPLETED.to_owned()],
                ..EventFilter::default()
            },
        )
        .expect("read event history");
    assert_eq!(latest, vec![event]);
}
