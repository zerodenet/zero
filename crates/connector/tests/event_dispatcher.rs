#![cfg(feature = "sink-jsonl")]

use std::fs;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::json;
use zero_api::{event_type, ApiEvent, EventFilter, EventSource, RawApiEvent};
use zero_config::{ApiConfig, EventSinkConfig};
use zero_connector::{spawn_event_dispatcher, EventDispatcherOptions};

#[derive(Clone)]
struct StaticEventSource {
    events: Arc<Mutex<Vec<RawApiEvent>>>,
}

impl EventSource for StaticEventSource {
    type Stream = Vec<RawApiEvent>;

    fn subscribe(&self, _filter: EventFilter) -> zero_api::ApiResult<Self::Stream> {
        Ok(self.events.lock().expect("events lock").clone())
    }
}

#[tokio::test]
async fn dispatcher_writes_matching_events_to_jsonl_sink() {
    let path = temp_path("zero-connector-events.jsonl");
    let _ = fs::remove_file(&path);

    let mut event = ApiEvent::new(
        "event-1",
        event_type::FLOW_COMPLETED,
        1_760_000_000_000,
        json!({ "value": 1 }),
    );
    event.sequence = Some(1);
    let source = StaticEventSource {
        events: Arc::new(Mutex::new(vec![event])),
    };
    let api = ApiConfig {
        event_sinks: vec![EventSinkConfig::JsonLines {
            tag: "local-events".to_owned(),
            path: path.display().to_string(),
            events: vec![event_type::FLOW_COMPLETED.to_owned()],
            source_id: Some("test-source".to_owned()),
        }],
        control: Default::default(),
    };

    let dispatcher = spawn_event_dispatcher(
        source,
        api,
        None,
        EventDispatcherOptions {
            poll_interval: Duration::from_millis(10),
            max_retry_attempts: 1,
        },
    )
    .expect("spawn dispatcher")
    .expect("dispatcher handle");

    let written = wait_for_file_contains(&path, "event-1").await;
    dispatcher.shutdown().await;
    let _ = fs::remove_file(&path);

    let line = written.lines().next().expect("jsonl line");
    let value = serde_json::from_str::<serde_json::Value>(line).expect("event json");
    assert_eq!(value["event_id"], "event-1");
    assert_eq!(value["source_id"], "test-source");
}

async fn wait_for_file_contains(path: &std::path::Path, needle: &str) -> String {
    for _ in 0..50 {
        if let Ok(content) = fs::read_to_string(path) {
            if content.contains(needle) {
                return content;
            }
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    panic!("file did not contain `{needle}`");
}

fn temp_path(name: &str) -> std::path::PathBuf {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    std::env::temp_dir().join(format!("{now}-{name}"))
}
