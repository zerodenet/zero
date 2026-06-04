use serde::{Deserialize, Serialize};

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

/// A response frame sent by the server to the client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcResponse {
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
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
            id: None,
            result: serde_json::to_value(result).ok(),
            error: None,
        }
    }

    pub fn ok_raw(result: serde_json::Value) -> Self {
        Self {
            ok: true,
            id: None,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(code: &str, message: impl Into<String>) -> Self {
        Self {
            ok: false,
            id: None,
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
            id: None,
            result: None,
            error: Some(IpcErrorBody {
                code: error.code.as_code_str().to_owned(),
                message: error.message.clone(),
                field_path: error.field_path.clone(),
            }),
        }
    }

    /// Attach a request id for response-request pairing on
    /// multiplexed connections.
    pub fn with_id(mut self, id: Option<u64>) -> Self {
        self.id = id;
        self
    }
}

/// Serialize a frame to a JSON line (with trailing newline).
pub fn serialize_frame(frame: &impl Serialize) -> Result<Vec<u8>, serde_json::Error> {
    let mut bytes = serde_json::to_vec(frame)?;
    bytes.push(b'\n');
    Ok(bytes)
}
