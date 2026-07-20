#![cfg(feature = "sink_jsonl")]

use std::collections::VecDeque;
use std::fs;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::json;
use zero_api::{event_type, ApiEvent, EventFilter, EventSource, EventStream, RawApiEvent};
use zero_config::{ApiConfig, EventSinkConfig};
use zero_connector::{spawn_event_dispatcher, EventDispatcherOptions};

#[derive(Clone)]
struct StaticEventSource {
    events: Arc<Mutex<Vec<RawApiEvent>>>,
}

struct StaticEventStream {
    events: Mutex<VecDeque<RawApiEvent>>,
}

impl EventStream for StaticEventStream {
    fn recv(&self) -> Option<RawApiEvent> {
        self.try_recv()
    }

    fn try_recv(&self) -> Option<RawApiEvent> {
        self.events.lock().expect("events lock").pop_front()
    }
}

impl EventSource for StaticEventSource {
    type Stream = StaticEventStream;

    fn subscribe(&self, _filter: EventFilter) -> zero_api::ApiResult<Self::Stream> {
        Ok(StaticEventStream {
            events: Mutex::new(self.events.lock().expect("events lock").clone().into()),
        })
    }

    fn latest(&self, limit: usize, _filter: EventFilter) -> zero_api::ApiResult<Vec<RawApiEvent>> {
        Ok(self
            .events
            .lock()
            .expect("events lock")
            .iter()
            .take(limit)
            .cloned()
            .collect())
    }

    fn since(
        &self,
        sequence: u64,
        limit: usize,
        _filter: EventFilter,
    ) -> zero_api::ApiResult<zero_api::EventReplay> {
        let events: Vec<_> = self
            .events
            .lock()
            .expect("events lock")
            .iter()
            .filter(|event| event.sequence.is_some_and(|value| value > sequence))
            .take(limit)
            .cloned()
            .collect();
        let actual_from = events
            .first()
            .and_then(|event| event.sequence)
            .unwrap_or_else(|| sequence.saturating_add(1));
        Ok(zero_api::EventReplay {
            requested_after: sequence,
            actual_from,
            has_gap: actual_from > sequence.saturating_add(1),
            events,
        })
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
    let mut snapshot = ApiEvent::new(
        "snapshot-0",
        event_type::FLOW_SNAPSHOT,
        1_760_000_000_000,
        json!({ "watermark": 0, "records": [] }),
    );
    snapshot.sequence = Some(0);
    let source = StaticEventSource {
        events: Arc::new(Mutex::new(vec![snapshot, event])),
    };
    let api = ApiConfig {
        event_sinks: vec![EventSinkConfig::JsonLines {
            tag: "local-events".to_owned(),
            path: path.display().to_string(),
            events: Vec::new(),
            source_id: Some("test-source".to_owned()),
        }],
        control: Default::default(),
        ..Default::default()
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

    let status_handle = dispatcher.status_handle();
    let written = wait_for_file_contains(&path, "event-1").await;
    dispatcher.shutdown().await;
    let sink_status = status_handle.sink_status();
    let _ = fs::remove_file(&path);

    assert_eq!(sink_status.len(), 1);
    assert_eq!(sink_status[0].name, "local-events");
    assert!(sink_status[0].total_delivered >= 1);

    let line = written.lines().next().expect("jsonl line");
    let value = serde_json::from_str::<serde_json::Value>(line).expect("event json");
    assert_eq!(value["event_id"], "event-1");
    assert_eq!(value["source_id"], "test-source");
    assert!(!written.contains("snapshot-0"));
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
