//! Unified response envelope shared by all control plane channels.
//!
//! `ApiResponse<T>` is used server-side to construct typed responses.
//! `RawResponse` is used by IPC clients to deserialize responses.
//! `EnvelopeError` is the shared error body format.

use serde::{Deserialize, Serialize};

use crate::{ApiError, API_VERSION};

// ── Error body ──────────────────────────────────────────────────────

/// Unified error body for all control plane channels.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvelopeError {
    /// Machine-readable error code (kebab-case).
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
/// The `api_version` field is always included for protocol identification.
#[derive(Debug, Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub api_version: &'static str,
    /// Request correlation ID for multiplexed connections (IPC) or
    /// request tracing (HTTP).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<EnvelopeError>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn ok(result: T) -> Self {
        Self {
            api_version: API_VERSION,
            id: None,
            ok: true,
            result: Some(result),
            error: None,
        }
    }

    pub fn ok_with_id(id: Option<u64>, result: T) -> Self {
        Self {
            api_version: API_VERSION,
            id,
            ok: true,
            result: Some(result),
            error: None,
        }
    }

    pub fn with_id(mut self, id: Option<u64>) -> Self {
        self.id = id;
        self
    }

    /// Construct an error response from an `ApiError`.
    ///
    /// The type parameter is `()` because error responses carry no result.
    /// Callers should use `ApiResponse::<()>::from_api_error(...)`.
    pub fn from_api_error(error: &ApiError) -> ApiResponse<()> {
        ApiResponse {
            api_version: API_VERSION,
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
            api_version: API_VERSION,
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
    pub api_version: Option<String>,
    #[serde(default)]
    pub id: Option<u64>,
    pub ok: bool,
    #[serde(default)]
    pub result: Option<serde_json::Value>,
    #[serde(default)]
    pub error: Option<EnvelopeError>,
}
