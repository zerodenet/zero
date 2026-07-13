use std::error::Error;

use crate::cli::Command;

pub async fn execute(command: Command) -> Result<(), Box<dyn Error>> {
    let Command::Run {
        config_path,
        status_listen,
        control_socket,
        ipc_hook_socket,
    } = command
    else {
        unreachable!("application routes only run commands here")
    };
    crate::run_command(
        &config_path,
        status_listen.as_deref(),
        control_socket.as_deref(),
        ipc_hook_socket.as_deref(),
    )
    .await
}
