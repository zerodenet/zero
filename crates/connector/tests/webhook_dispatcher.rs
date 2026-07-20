#![cfg(feature = "panel_connector")]

use std::collections::VecDeque;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

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
async fn dispatcher_posts_events_to_panel_webhook_with_api_key() {
    let (url, server) = spawn_http_server();

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
        event_sinks: vec![EventSinkConfig::Webhook {
            tag: "panel".to_owned(),
            url,
            events: vec![event_type::FLOW_COMPLETED.to_owned()],
            source_id: Some("edge-test".to_owned()),
            api_key: Some("panel-key".to_owned()),
            api_key_env: None,
            allow_insecure: true,
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

    let request = server.join().expect("server thread");
    dispatcher.shutdown().await;

    assert!(request.starts_with("POST /events HTTP/1.1\r\n"));
    assert!(request
        .to_ascii_lowercase()
        .contains("authorization: bearer panel-key"));
    let body = request_body(&request);
    let value = serde_json::from_str::<serde_json::Value>(body).expect("event json");
    assert_eq!(value["event_id"], "event-1");
    assert_eq!(value["source_id"], "edge-test");
}

fn spawn_http_server() -> (String, JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local server");
    listener
        .set_nonblocking(true)
        .expect("set nonblocking listener");
    let address = listener.local_addr().expect("local address");

    let handle = thread::spawn(move || {
        for _ in 0..100 {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    stream
                        .set_read_timeout(Some(Duration::from_secs(5)))
                        .expect("read timeout");
                    let request = read_http_request(&mut stream);
                    stream
                        .write_all(
                            b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                        )
                        .expect("write response");
                    return request;
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(20));
                }
                Err(error) => panic!("accept request: {error}"),
            }
        }

        panic!("webhook request was not received");
    });

    (format!("http://{address}/events"), handle)
}

fn read_http_request(stream: &mut TcpStream) -> String {
    let mut received = Vec::new();
    let mut buffer = [0_u8; 1024];
    let header_end = loop {
        let read = stream.read(&mut buffer).expect("read request");
        assert!(read > 0, "connection closed before headers completed");
        received.extend_from_slice(&buffer[..read]);
        if let Some(position) = received.windows(4).position(|window| window == b"\r\n\r\n") {
            break position + 4;
        }
    };

    let headers = String::from_utf8_lossy(&received[..header_end]);
    let content_length = headers
        .lines()
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.trim().parse::<usize>().ok())
        .expect("content-length header");

    while received.len() < header_end + content_length {
        let read = stream.read(&mut buffer).expect("read request body");
        assert!(read > 0, "connection closed before body completed");
        received.extend_from_slice(&buffer[..read]);
    }

    String::from_utf8(received).expect("utf-8 request")
}

fn request_body(request: &str) -> &str {
    request
        .split_once("\r\n\r\n")
        .map(|(_, body)| body)
        .expect("request body")
}
