pub mod ipc;

use std::sync::Arc;
use std::time::Duration;

use zero_config::{ApiConfig, HookConfig};
use zero_engine::FlowHookChain;

type WarningHandler = Arc<dyn Fn(&str, &str) + Send + Sync>;

/// Build the hook chain from CLI option and config file.
///
/// CLI `ipc_socket` takes precedence.  Config `api.hooks` entries are
/// added after the CLI hook.
pub fn build_hook_chain(
    ipc_socket: Option<&str>,
    api_config: &ApiConfig,
    on_warning: Option<WarningHandler>,
) -> FlowHookChain {
    let mut chain = FlowHookChain::empty();

    // CLI override: single IPC hook.
    if let Some(socket_path) = ipc_socket {
        push_ipc_hook(&mut chain, socket_path, 100, &on_warning);
    }

    // Config-based hooks (only if no CLI override for IPC).
    for hook_cfg in &api_config.hooks {
        match hook_cfg {
            HookConfig::Ipc { socket, timeout_ms } => {
                if ipc_socket.is_none() {
                    push_ipc_hook(&mut chain, socket, *timeout_ms, &on_warning);
                }
            }
        }
    }

    chain
}

fn push_ipc_hook(
    chain: &mut FlowHookChain,
    socket_path: &str,
    timeout_ms: u64,
    on_warning: &Option<WarningHandler>,
) {
    let mut hook = ipc::IpcFlowHook::new(socket_path, Duration::from_millis(timeout_ms));
    if let Some(handler) = on_warning {
        let h = Arc::clone(handler);
        hook = hook.with_warning_handler(move |code, msg| h(code, msg));
    }
    chain.push(Arc::new(hook));
    tracing::info!(socket = %socket_path, timeout_ms, "ipc flow hook enabled");
}
