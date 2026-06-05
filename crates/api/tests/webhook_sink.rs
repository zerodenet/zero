#![cfg(feature = "webhook")]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use serde_json::{json, Value};
use zero_api::{
    event_type, ApiErrorCode, ApiEvent, EventSink, RawApiEvent, WebhookEventSink,
    WebhookEventSinkConfig,
};

fn event(id: &str, event_type: &str) -> RawApiEvent {
    ApiEvent::new(id, event_type, 1_760_000_000_000, json!({ "value": id }))
}

#[test]
fn webhook_event_sink_posts_normalized_json_event() {
    let (url, handle) = spawn_http_server("204 No Content");
    let sink = WebhookEventSink::with_config(
        WebhookEventSinkConfig::new(url).with_header("x-zero-token", "test-token"),
    )
    .expect("webhook sink");

    let result = sink
        .publish(&event("event-1", event_type::FLOW_COMPLETED))
        .expect("publish");

    assert!(result.delivered);
    assert!(!result.retryable);
    assert_eq!(result.message, None);

    let request = handle.join().expect("server thread");
    assert!(request.starts_with("POST /events HTTP/1.1\r\n"));
    assert!(request.to_ascii_lowercase().contains("x-zero-token:"));
    assert!(request.contains("test-token"));

    let body = request_body(&request);
    let posted = serde_json::from_str::<Value>(body).expect("posted event json");
    assert_eq!(posted["schema_id"], "zero.event.v1");
    assert_eq!(posted["event_id"], "event-1");
    assert_eq!(posted["event_type"], "flow.completed");
    assert_eq!(posted["payload"]["value"], "event-1");
}

#[test]
fn webhook_event_sink_marks_server_errors_retryable() {
    let (url, handle) = spawn_http_server("503 Service Unavailable");
    let sink = WebhookEventSink::new(url).expect("webhook sink");

    let result = sink
        .publish(&event("event-2", event_type::STATS_SAMPLED))
        .expect("publish");

    assert!(!result.delivered);
    assert!(result.retryable);
    assert_eq!(result.message, Some("webhook returned HTTP 503".to_owned()));
    handle.join().expect("server thread");
}

#[test]
fn webhook_event_sink_rejects_non_http_urls() {
    let error = WebhookEventSink::new("file:///tmp/zero-events.jsonl").expect_err("invalid url");

    assert_eq!(error.code, ApiErrorCode::InvalidArgument);
    assert_eq!(error.field_path.as_deref(), Some("url"));
}

fn spawn_http_server(status: &'static str) -> (String, JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local server");
    let address = listener.local_addr().expect("local address");
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept request");
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("read timeout");
        let request = read_http_request(&mut stream);
        let response =
            format!("HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        stream
            .write_all(response.as_bytes())
            .expect("write response");
        request
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
