//! IPC event emission helpers.
//!
//! Emits structured `ipc.connected` / `ipc.disconnected` events
//! via the engine event bus so GUI subscribers can observe IPC
//! connection lifecycle without parsing terminal logs.

use std::time::{SystemTime, UNIX_EPOCH};

use zero_api::event_type;
use zero_proxy::ProxyHandle;

/// Emit an `ipc.connected` event.
pub(crate) fn emit_connected(handle: &ProxyHandle, active: u64, pipe: &str) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let event = zero_api::ApiEvent::new(
        format!("ipc.connected:{now}"),
        event_type::IPC_CONNECTED,
        now,
        serde_json::json!({"active": active, "pipe": pipe}),
    );
    handle.engine_handle().emit(event);
}

/// Emit an `ipc.disconnected` event.
pub(crate) fn emit_disconnected(
    handle: &ProxyHandle,
    active: u64,
    pipe: &str,
    error: Option<&str>,
) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let mut payload = serde_json::json!({"active": active, "pipe": pipe});
    if let Some(e) = error {
        payload["error"] = serde_json::Value::String(e.to_owned());
    }
    let event = zero_api::ApiEvent::new(
        format!("ipc.disconnected:{now}"),
        event_type::IPC_DISCONNECTED,
        now,
        payload,
    );
    handle.engine_handle().emit(event);
}
