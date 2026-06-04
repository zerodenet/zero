use serde::{Deserialize, Serialize};

use zero_api::{ApiResponse, RawResponse};

/// A request frame sent by the client to the UDS server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IpcRequest {
    /// Execute a query.
    Query {
        #[serde(default)]
        id: Option<u64>,
        request: zero_api::QueryRequest,
    },
    /// Execute a command.
    Command {
        #[serde(default)]
        id: Option<u64>,
        method: String,
        params: serde_json::Value,
    },
    /// Subscribe to events (keeps the connection open).
    Subscribe {
        #[serde(default)]
        id: Option<u64>,
        events: Option<Vec<String>>,
    },
    /// Ping to verify the connection is alive.
    Ping {
        #[serde(default)]
        id: Option<u64>,
    },
}

impl IpcRequest {
    /// Extract the optional request id for response echo.
    pub fn id(&self) -> Option<u64> {
        match self {
            IpcRequest::Query { id, .. }
            | IpcRequest::Command { id, .. }
            | IpcRequest::Subscribe { id, .. }
            | IpcRequest::Ping { id } => *id,
        }
    }
}

/// An event frame pushed by the server to subscribed clients.
///
/// Regular events are sent as `zero_api::ApiEvent<serde_json::Value>` JSON
/// directly (the same format as SSE).  The `Goodbye` frame is IPC-specific.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IpcEvent {
    /// Server is shutting down; clients should reconnect.
    Goodbye { message: String },
}

// ── IPC server response helpers ─────────────────────────────────────

/// Construct a success response with a raw JSON value and optional request id.
pub fn ipc_ok(id: Option<u64>, result: impl Serialize) -> ApiResponse<serde_json::Value> {
    let value = serde_json::to_value(result).unwrap_or(serde_json::Value::Null);
    ApiResponse::ok_with_id(id, value)
}

/// Construct an error response with a code, message, and optional request id.
pub fn ipc_error(
    id: Option<u64>,
    code: impl Into<String>,
    message: impl Into<String>,
) -> ApiResponse<()> {
    ApiResponse::error_msg(code, message).with_id(id)
}

/// Construct an error response from an `ApiError`, with optional request id.
pub fn ipc_api_error(id: Option<u64>, error: &zero_api::ApiError) -> ApiResponse<()> {
    ApiResponse::<()>::from_api_error(error).with_id(id)
}

/// Serialize a frame to a JSON line (with trailing newline).
pub fn serialize_frame(frame: &impl Serialize) -> Result<Vec<u8>, serde_json::Error> {
    let mut bytes = serde_json::to_vec(frame)?;
    bytes.push(b'\n');
    Ok(bytes)
}

// ── Re-export for IPC client deserialization ─────────────────────────

/// The response type returned by IPC client operations.
/// Shares the same wire format as `ApiResponse<T>`.
pub type IpcResponse = RawResponse;
