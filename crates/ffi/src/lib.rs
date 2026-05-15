//! C-compatible FFI bindings for the Zero proxy engine.
//!
//! ## Quick start
//!
//! ```c
//! ZeroHandle* h = zero_start("/etc/zero/config.json");
//! char* status = zero_query(h, "{\"Runtime\":{}}");
//! printf("%s\n", status);
//! zero_free_string(status);
//! zero_shutdown(h);
//! ```
//!
//! ## Thread safety
//!
//! A `ZeroHandle` may be used from any thread.  Each call to `zero_start`
//! creates an independent engine instance.

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::sync::Mutex;

use tokio::runtime::Runtime;
use zero_api::{CommandRequest, CommandService, QueryRequest, QueryService};
use zero_engine::EngineHandle;

// ── Error handling ──

static LAST_ERROR: Mutex<Option<String>> = Mutex::new(None);

fn set_error(msg: impl Into<String>) {
    *LAST_ERROR.lock().unwrap() = Some(msg.into());
}

fn clear_error() {
    *LAST_ERROR.lock().unwrap() = None;
}

fn take_error() -> Option<String> {
    LAST_ERROR.lock().unwrap().take()
}

// ── Handle ──

pub struct ZeroHandle {
    rt: Runtime,
    engine: EngineHandle,
}

// ── FFI functions ──

/// Start the Zero proxy engine from a JSON configuration file.
///
/// Returns an opaque handle on success, or null on failure (call
/// `zero_last_error()` for details).
///
/// # Safety
/// `config_path` must be a valid, null-terminated UTF-8 C string.
#[no_mangle]
pub unsafe extern "C" fn zero_start(config_path: *const c_char) -> *mut ZeroHandle {
    clear_error();

    let path = match unsafe { CStr::from_ptr(config_path) }.to_str() {
        Ok(s) => s.to_owned(),
        Err(e) => {
            set_error(format!("config_path is not valid UTF-8: {e}"));
            return std::ptr::null_mut();
        }
    };

    let rt = match Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            set_error(format!("failed to create tokio runtime: {e}"));
            return std::ptr::null_mut();
        }
    };

    let handle = match rt.block_on(async { EngineHandle::from_path(&path) }) {
        Ok(h) => h,
        Err(e) => {
            set_error(format!("failed to load config: {e}"));
            return std::ptr::null_mut();
        }
    };

    Box::into_raw(Box::new(ZeroHandle { rt, engine: handle }))
}

/// Shutdown the engine and free all resources.
///
/// # Safety
/// `handle` must be a valid pointer from `zero_start`. Must NOT be used
/// after this call.
#[no_mangle]
pub unsafe extern "C" fn zero_shutdown(handle: *mut ZeroHandle) {
    if handle.is_null() {
        return;
    }
    let _h = unsafe { Box::from_raw(handle) };
}

/// Query engine state.
///
/// `request_json` is a JSON string matching `QueryRequest` from zero-api.
/// Returns a JSON response string (caller must free with
/// `zero_free_string`), or null on error.
///
/// # Safety
/// `handle` and `request_json` must be valid, non-null pointers.
#[no_mangle]
pub unsafe extern "C" fn zero_query(
    handle: *mut ZeroHandle,
    request_json: *const c_char,
) -> *mut c_char {
    clear_error();

    let h = unsafe { &*handle };
    let json = match unsafe { CStr::from_ptr(request_json) }.to_str() {
        Ok(s) => s,
        Err(e) => {
            set_error(format!("invalid UTF-8 in request: {e}"));
            return std::ptr::null_mut();
        }
    };

    let request: QueryRequest = match serde_json::from_str(json) {
        Ok(r) => r,
        Err(e) => {
            set_error(format!("invalid query: {e}"));
            return std::ptr::null_mut();
        }
    };

    let response = h.rt.block_on(async { h.engine.query(request) });
    into_c_string(serde_json::to_string(&response).ok())
}

/// Execute a command.
///
/// `command_json` is a JSON string matching `CommandRequest` from
/// zero-api.  Returns a JSON response string (caller must free with
/// `zero_free_string`), or null on error.
///
/// # Safety
/// `handle` and `command_json` must be valid, non-null pointers.
#[no_mangle]
pub unsafe extern "C" fn zero_execute(
    handle: *mut ZeroHandle,
    command_json: *const c_char,
) -> *mut c_char {
    clear_error();

    let h = unsafe { &*handle };
    let json = match unsafe { CStr::from_ptr(command_json) }.to_str() {
        Ok(s) => s,
        Err(e) => {
            set_error(format!("invalid UTF-8 in command: {e}"));
            return std::ptr::null_mut();
        }
    };

    let command: CommandRequest = match serde_json::from_str(json) {
        Ok(c) => c,
        Err(e) => {
            set_error(format!("invalid command: {e}"));
            return std::ptr::null_mut();
        }
    };

    let response = h.rt.block_on(async { h.engine.execute(command) });
    into_c_string(serde_json::to_string(&response).ok())
}

/// Free a string returned by a `zero_*` function.
///
/// # Safety
/// `s` must be a pointer from `zero_query`, `zero_execute`, or
/// `zero_last_error`, and must not have been freed already.
#[no_mangle]
pub unsafe extern "C" fn zero_free_string(s: *mut c_char) {
    if !s.is_null() {
        unsafe { let _ = CString::from_raw(s); }
    }
}

/// Get the last error message for this thread.
///
/// Returns a human-readable string (caller must free with
/// `zero_free_string`), or null if no error.  The error state is cleared
/// after this call.
#[no_mangle]
pub extern "C" fn zero_last_error() -> *mut c_char {
    take_error().map_or(std::ptr::null_mut(), |msg| {
        CString::new(msg).unwrap_or_default().into_raw()
    })
}

// ── helpers ──

fn into_c_string(s: Option<String>) -> *mut c_char {
    s.map_or(std::ptr::null_mut(), |s| {
        CString::new(s).unwrap_or_default().into_raw()
    })
}
