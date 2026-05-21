use serde::{Deserialize, Serialize};

/// A request frame sent by the client to the UDS server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IpcRequest {
    /// Execute a query.
    Query { request: zero_api::QueryRequest },
    /// Execute a command.
    Command {
        method: String,
        params: serde_json::Value,
    },
    /// Subscribe to events (keeps the connection open).
    Subscribe { events: Option<Vec<String>> },
    /// Ping to verify the connection is alive.
    Ping,
}

/// A response frame sent by the server to the client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcResponse {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<IpcErrorBody>,
}

/// An event frame pushed by the server to subscribed clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IpcEvent {
    Event {
        event_type: String,
        event_id: String,
        occurred_at_unix_ms: u64,
        payload: serde_json::Value,
    },
    /// Server is shutting down.
    Goodbye { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcErrorBody {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field_path: Option<String>,
}

impl IpcResponse {
    pub fn ok(result: impl Serialize) -> Self {
        Self {
            ok: true,
            result: serde_json::to_value(result).ok(),
            error: None,
        }
    }

    pub fn ok_raw(result: serde_json::Value) -> Self {
        Self {
            ok: true,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(code: &str, message: impl Into<String>) -> Self {
        Self {
            ok: false,
            result: None,
            error: Some(IpcErrorBody {
                code: code.to_owned(),
                message: message.into(),
                field_path: None,
            }),
        }
    }

    pub fn from_api_error(error: &zero_api::ApiError) -> Self {
        Self {
            ok: false,
            result: None,
            error: Some(IpcErrorBody {
                code: error.code.as_code_str().to_owned(),
                message: error.message.clone(),
                field_path: error.field_path.clone(),
            }),
        }
    }
}

/// Serialize a frame to a JSON line (with trailing newline).
pub fn serialize_frame(frame: &impl Serialize) -> Result<Vec<u8>, serde_json::Error> {
    let mut bytes = serde_json::to_vec(frame)?;
    bytes.push(b'\n');
    Ok(bytes)
}
