use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use tokio::net::UnixListener;
use tokio::sync::oneshot;
use tokio::task::JoinSet;
use tracing::{debug, error, info, warn};
use zero_proxy::ProxyHandle;

use super::connection;

/// Default control socket path relative to $HOME.
pub const DEFAULT_SOCKET_REL_PATH: &str = ".zero/control.sock";

pub struct IpcServerHandle {
    shutdown: Option<oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<io::Result<()>>,
    socket_path: PathBuf,
}

impl IpcServerHandle {
    pub async fn shutdown(mut self) -> io::Result<()> {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        let result = self.task.await.expect("ipc server task should join");
        let _ = std::fs::remove_file(&self.socket_path);
        result
    }
}

/// Compute the default socket path: `$HOME/.zero/control.sock`.
pub fn default_socket_path() -> Option<PathBuf> {
    let home = dirs_home()?;
    Some(home.join(DEFAULT_SOCKET_REL_PATH))
}

/// Resolve the socket path in order of precedence:
///
/// 1. Explicit path from CLI `--control-socket`.
/// 2. `{exe_dir}/control.sock` — sibling to the executable.
/// 3. `$HOME/.zero/control.sock` — global fallback.
pub fn resolve_socket_path(explicit: Option<&str>) -> io::Result<PathBuf> {
    if let Some(path) = explicit {
        return Ok(PathBuf::from(path));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            return Ok(dir.join("control.sock"));
        }
    }
    default_socket_path().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "cannot determine home directory for control socket",
        )
    })
}

/// Spawn the IPC server on a Unix domain socket.
///
/// The socket file is created with `0o600` permissions so only the owning
/// user can connect.  If the socket file already exists it is removed first.
pub async fn spawn_ipc_server(
    handle: ProxyHandle,
    socket_path: &Path,
) -> io::Result<IpcServerHandle> {
    if socket_path.exists() {
        std::fs::remove_file(socket_path)?;
    }

    if let Some(parent) = socket_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let listener = UnixListener::bind(socket_path)?;
    let metadata = socket_path.metadata()?;
    let mut perms = metadata.permissions();
    perms.set_mode(0o600);
    std::fs::set_permissions(socket_path, perms)?;

    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let path_for_handle = socket_path.to_path_buf();

    let task = tokio::spawn(async move { run_ipc_server(listener, handle, shutdown_rx).await });

    info!(socket = %socket_path.display(), "ipc server ready");

    Ok(IpcServerHandle {
        shutdown: Some(shutdown_tx),
        task,
        socket_path: path_for_handle,
    })
}

async fn run_ipc_server(
    listener: UnixListener,
    handle: ProxyHandle,
    mut shutdown: oneshot::Receiver<()>,
) -> io::Result<()> {
    let mut connections = JoinSet::new();

    loop {
        tokio::select! {
            _ = &mut shutdown => break,
            accept_result = listener.accept() => {
                let (stream, peer_addr) = accept_result?;
                let handle = handle.clone();
                connections.spawn(async move {
                    if let Err(error) = connection::handle_ipc_connection(stream, handle).await {
                        if connection::is_transient_disconnect(&error) {
                            debug!(?peer_addr, error = %error, "ipc connection closed early");
                        } else {
                            warn!(?peer_addr, error = %error, "ipc connection failed");
                        }
                    }
                });
            }
            result = connections.join_next(), if !connections.is_empty() => {
                if let Some(Err(error)) = result {
                    if !error.is_cancelled() {
                        error!(error = %error, "ipc connection task panicked");
                    }
                }
            }
        }
    }

    connections.abort_all();
    while let Some(result) = connections.join_next().await {
        if let Err(error) = result {
            if !error.is_cancelled() {
                error!(error = %error, "ipc connection task panicked during shutdown");
            }
        }
    }

    info!("ipc server stopped");
    Ok(())
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
}
