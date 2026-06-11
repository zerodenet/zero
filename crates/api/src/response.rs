//! Unified response envelope shared by all control plane channels.
//!
//! `ApiResponse<T>` is used server-side to construct typed responses.
//! `RawResponse` is used by IPC clients to deserialize responses.
//! `EnvelopeError` is the shared error body format.

use serde::{Deserialize, Serialize};

use crate::{ApiError, API_ID};

// ── Error body ──────────────────────────────────────────────────────

/// Unified error body for all control plane channels.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvelopeError {
    /// Machine-readable error code (snake_case).
    pub code: String,
    /// Human-readable error message.
    pub message: String,
    /// Parameter field path for validation errors.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field_path: Option<String>,
}

impl EnvelopeError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            field_path: None,
        }
    }
}

// ── Generic response envelope (server-side, Serialize) ──────────────

/// Generic response envelope for all control plane channels.
///
/// Both HTTP and IPC servers construct responses using this type.
/// The `api_id` field is always included for protocol identification.
#[derive(Debug, Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub api_id: &'static str,
    /// Request correlation ID for multiplexed connections (IPC) or
    /// request tracing (HTTP).
    ///
    /// Opaque to the server: whatever JSON value the client sent (number,
    /// string, null) is echoed back verbatim, so clients may use any
    /// correlation scheme — numeric counters, UUIDs, labels, etc.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<serde_json::Value>,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<EnvelopeError>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn ok(result: T) -> Self {
        Self {
            api_id: API_ID,
            id: None,
            ok: true,
            result: Some(result),
            error: None,
        }
    }

    pub fn ok_with_id(id: Option<serde_json::Value>, result: T) -> Self {
        Self {
            api_id: API_ID,
            id,
            ok: true,
            result: Some(result),
            error: None,
        }
    }

    pub fn with_id(mut self, id: Option<serde_json::Value>) -> Self {
        self.id = id;
        self
    }

    /// Construct an error response from an `ApiError`.
    ///
    /// The type parameter is `()` because error responses carry no result.
    /// Callers should use `ApiResponse::<()>::from_api_error(...)`.
    pub fn from_api_error(error: &ApiError) -> ApiResponse<()> {
        ApiResponse {
            api_id: API_ID,
            id: None,
            ok: false,
            result: None,
            error: Some(EnvelopeError {
                code: error.code.as_code_str().to_owned(),
                message: error.message.clone(),
                field_path: error.field_path.clone(),
            }),
        }
    }
}

// ── Error-only convenience constructors ─────────────────────────────

impl ApiResponse<()> {
    /// Construct an error response from a code string and message.
    pub fn error_msg(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            api_id: API_ID,
            id: None,
            ok: false,
            result: None,
            error: Some(EnvelopeError::new(code, message)),
        }
    }
}

// ── Raw response (client-side, Deserialize) ─────────────────────────

/// Deserializable response for IPC client consumption.
///
/// Shares the same wire format as `ApiResponse<T>` but uses
/// `serde_json::Value` so the client doesn't need the concrete type.
#[derive(Debug, Clone, Deserialize)]
pub struct RawResponse {
    #[serde(default)]
    pub api_id: Option<String>,
    #[serde(default)]
    pub id: Option<serde_json::Value>,
    pub ok: bool,
    #[serde(default)]
    pub result: Option<serde_json::Value>,
    #[serde(default)]
    pub error: Option<EnvelopeError>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
}
