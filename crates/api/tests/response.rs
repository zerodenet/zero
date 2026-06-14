//! Tests for the unified response envelope (`ApiResponse`, `RawResponse`).
//!
//! Migrated from the inline `#[cfg(test)] mod tests` in
//! `crates/api/src/response.rs`.

use serde_json::json;
use zero_api::{ApiResponse, RawResponse};

// The request `id` is an opaque correlation token: the server must echo
// back whatever the client sent (string, number, or null) verbatim, with
// no numeric coercion or truncation. These guard the `Option<Value>` type
// against ever being re-tightened to a numeric type.

#[test]
fn string_id_is_echoed_verbatim() {
    let resp = ApiResponse::<()>::error_msg("code", "msg").with_id(Some(json!("znet-sink-1")));
    let v = serde_json::to_value(&resp).unwrap();
    assert_eq!(v["id"], json!("znet-sink-1"));
    assert!(v["id"].is_string());
}

#[test]
fn numeric_id_is_echoed_verbatim() {
    let resp = ApiResponse::<()>::error_msg("code", "msg").with_id(Some(json!(42)));
    let v = serde_json::to_value(&resp).unwrap();
    assert_eq!(v["id"], json!(42));
    assert!(v["id"].is_number());
}

#[test]
fn null_and_absent_id_round_trip() {
    let explicit = ApiResponse::<()>::error_msg("code", "msg").with_id(Some(json!(null)));
    let v = serde_json::to_value(&explicit).unwrap();
    assert_eq!(v["id"], json!(null));

    let absent = ApiResponse::<()>::error_msg("code", "msg"); // id left None
    let v = serde_json::to_value(&absent).unwrap();
    assert!(v.get("id").is_none(), "absent id must be skipped, not null");
}

#[test]
fn raw_response_deserializes_string_id() {
    // A response echoing a client's string id must parse on the client side.
    let wire = r#"{"api_id":"zero","id":"znet-sink-1","ok":true,"result":"x"}"#;
    let r: RawResponse = serde_json::from_str(wire).unwrap();
    assert_eq!(r.id, Some(json!("znet-sink-1")));
    assert!(r.ok);
}
