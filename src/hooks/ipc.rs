use std::io::{self, BufRead, BufReader, Write};
use std::sync::Mutex;
use std::time::Duration;

use zero_engine::{BlockReason, FlowContext, FlowHook, FlowTraffic, SessionOutcome};

// Platform-specific stream type.
#[cfg(unix)]
type IpcStream = std::os::unix::net::UnixStream;
#[cfg(windows)]
type IpcStream = std::fs::File;

/// An `IpcFlowHook` delegates flow decisions to an external process over
/// a local IPC channel (Unix domain socket or Windows named pipe).
///
/// # Protocol (JSON-line)
///
/// **flow_start** — synchronous request/response:
/// ```text
/// → {"type":"check_flow","flow_id":1,...}
/// ← {"allow":true}
/// ← {"allow":false,"code":"quota","message":"daily quota exceeded"}
/// ```
///
/// **flow_end** — fire-and-forget:
/// ```text
/// → {"type":"flow_end","flow_id":1,...}
/// ```
///
/// Fail-open: if the external process is unreachable, flows are allowed.
pub struct IpcFlowHook {
    path: String,
    timeout: Duration,
    stream: Mutex<Option<IpcStream>>,
    on_warning: Option<Box<dyn Fn(&str, &str) + Send + Sync>>,
}

impl IpcFlowHook {
    pub fn new(path: impl Into<String>, timeout: Duration) -> Self {
        Self {
            path: path.into(),
            timeout,
            stream: Mutex::new(None),
            on_warning: None,
        }
    }

    pub fn with_warning_handler(
        mut self,
        handler: impl Fn(&str, &str) + Send + Sync + 'static,
    ) -> Self {
        self.on_warning = Some(Box::new(handler));
        self
    }

    fn emit_warning(&self, code: &str, message: &str) {
        if let Some(ref handler) = self.on_warning {
            handler(code, message);
        }
    }

    fn send_recv(&self, request: &serde_json::Value) -> io::Result<serde_json::Value> {
        let request_line = serde_json::to_string(request).map_err(io::Error::other)?;
        let request_bytes = format!("{request_line}\n");

        let mut stream = self.connect()?;
        set_timeout(&mut stream, Some(self.timeout))?;

        stream.write_all(request_bytes.as_bytes())?;
        stream.flush()?;

        let reader = BufReader::new(&stream);
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            return serde_json::from_str(&line).map_err(io::Error::other);
        }

        Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "ipc hook closed connection without response",
        ))
    }

    fn send_fire_and_forget(&self, notification: &serde_json::Value) {
        let Ok(line) = serde_json::to_string(notification) else {
            return;
        };
        let bytes = format!("{line}\n").into_bytes();

        if let Ok(mut stream) = self.connect() {
            let _ = set_timeout(&mut stream, Some(Duration::from_secs(2)));
            let _ = stream.write_all(&bytes);
        }
    }

    fn connect(&self) -> io::Result<IpcStream> {
        // Try cached connection.
        {
            let mut guard = self.stream.lock().expect("ipc hook lock poisoned");
            if let Some(stream) = guard.take() {
                if is_alive(&stream) {
                    set_timeout(guard.as_mut().unwrap(), Some(self.timeout))?;
                    return Ok(stream);
                }
            }
        }

        let stream = connect_ipc(&self.path)?;

        // Cache for next call.
        {
            let mut guard = self.stream.lock().expect("ipc hook lock poisoned");
            let cloned = clone_stream(&stream)?;
            let _ = guard.insert(cloned);
        }

        Ok(stream)
    }
}

impl FlowHook for IpcFlowHook {
    fn on_flow_start(&self, ctx: &FlowContext) -> Result<(), BlockReason> {
        let request = serde_json::json!({
            "type": "check_flow",
            "flow_id": ctx.flow_id,
            "inbound_tag": ctx.inbound_tag,
            "target_host": ctx.target_host,
            "target_port": ctx.target_port,
            "network": ctx.network,
            "protocol": ctx.protocol,
            "auth_scheme": ctx.auth.as_ref().map(|a| &a.scheme),
            "principal_key": ctx.auth.as_ref().and_then(|a| a.principal_key.as_deref()),
            "mode": ctx.mode,
        });

        match self.send_recv(&request) {
            Ok(response) => {
                if response
                    .get("allow")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true)
                {
                    Ok(())
                } else {
                    let code = response
                        .get("code")
                        .and_then(|v| v.as_str())
                        .unwrap_or("blocked")
                        .to_owned();
                    let message = response
                        .get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("blocked by external hook")
                        .to_owned();
                    Err(BlockReason::new(code, message))
                }
            }
            Err(error) => {
                let msg = format!("ipc hook unreachable ({error}); allowing flow (fail-open)");
                tracing::warn!(path = %self.path, error = %error, "{}", msg);
                self.emit_warning("ipc_hook_unreachable", &msg);
                let _ = self.stream.lock().expect("ipc hook lock poisoned").take();
                Ok(())
            }
        }
    }

    fn on_flow_end(&self, ctx: &FlowContext, outcome: SessionOutcome, stats: &FlowTraffic) {
        let notification = serde_json::json!({
            "type": "flow_end",
            "flow_id": ctx.flow_id,
            "inbound_tag": ctx.inbound_tag,
            "target_host": ctx.target_host,
            "target_port": ctx.target_port,
            "network": ctx.network,
            "principal_key": ctx.auth.as_ref().and_then(|a| a.principal_key.as_deref()),
            "outcome": outcome.kind(),
            "bytes_up": stats.bytes_up,
            "bytes_down": stats.bytes_down,
            "duration_ms": stats.duration_ms,
        });
        self.send_fire_and_forget(&notification);
    }
}

// ── Platform-specific helpers ─────────────────────────────────────────

#[cfg(unix)]
use std::os::unix::net::UnixStream;

#[cfg(unix)]
fn connect_ipc(path: &str) -> io::Result<IpcStream> {
    UnixStream::connect(path)
}

#[cfg(unix)]
fn clone_stream(stream: &IpcStream) -> io::Result<IpcStream> {
    stream.try_clone()
}

#[cfg(unix)]
fn is_alive(_stream: &IpcStream) -> bool {
    // Unix socket connections are cheap to recreate; don't bother with
    // liveness checking.  Always reconnect.
    false
}

#[cfg(unix)]
fn set_timeout(stream: &mut IpcStream, timeout: Option<Duration>) -> io::Result<()> {
    stream.set_read_timeout(timeout)?;
    stream.set_write_timeout(timeout)
}

#[cfg(windows)]
fn connect_ipc(path: &str) -> io::Result<IpcStream> {
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
}

#[cfg(windows)]
fn clone_stream(stream: &IpcStream) -> io::Result<IpcStream> {
    stream.try_clone()
}

#[cfg(windows)]
fn is_alive(_stream: &IpcStream) -> bool {
    // On Windows, reconnection is cheap — just reconnect.
    false
}

#[cfg(windows)]
fn set_timeout(_stream: &mut IpcStream, _timeout: Option<Duration>) -> io::Result<()> {
    // Windows File handles don't support read/write timeouts natively.
    // The connection is local, so timeouts are not critical.
    Ok(())
}
