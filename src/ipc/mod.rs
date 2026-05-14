pub mod client;
pub mod protocol;

// Platform-specific IPC server implementation.
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

/// Default IPC path for the current platform.
pub fn default_ipc_path() -> Option<std::path::PathBuf> {
    #[cfg(unix)]
    {
        unix::default_socket_path()
    }
    #[cfg(windows)]
    {
        windows::default_ipc_path()
    }
}

/// Resolve the IPC path: explicit override, or platform default.
pub fn resolve_ipc_path(explicit: Option<&str>) -> std::io::Result<std::path::PathBuf> {
    #[cfg(unix)]
    {
        unix::resolve_socket_path(explicit)
    }
    #[cfg(windows)]
    {
        windows::resolve_ipc_path(explicit)
    }
}

// Re-export the platform-specific server handle and spawn function.
#[cfg(unix)]
pub use unix::{spawn_ipc_server, IpcServerHandle};
#[cfg(windows)]
pub use windows::{spawn_ipc_server, IpcServerHandle};
