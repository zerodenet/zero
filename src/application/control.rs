use std::error::Error;

use crate::cli::Command;

pub fn execute(command: Command) -> Result<(), Box<dyn Error>> {
    match command {
        Command::Select {
            policy_tag,
            target_tag,
            socket_path,
        } => crate::select_command(&policy_tag, &target_tag, socket_path.as_deref()),
        Command::Flows { socket_path } => crate::flows_command(socket_path.as_deref()),
        Command::Policies { socket_path } => crate::policies_command(socket_path.as_deref()),
        Command::Events { socket_path } => crate::events_command(socket_path.as_deref()),
        Command::Reload {
            config_path,
            socket_path,
        } => crate::reload_command(&config_path, socket_path.as_deref()),
        Command::Mode {
            mode,
            outbound,
            socket_path,
        } => {
            let socket = crate::resolve_socket(socket_path.as_deref())?;
            let request = crate::ipc::protocol::IpcRequest::Command {
                id: None,
                method: "mode.set".to_owned(),
                params: serde_json::json!({"mode": mode, "outbound": outbound}),
            };
            let response = crate::ipc::client::send_request(&socket, &request)?;
            if !response.ok {
                return Err(response
                    .error
                    .map(|error| error.message)
                    .unwrap_or_default()
                    .into());
            }
            let target = outbound.map_or(mode, |tag| format!("global -> {tag}"));
            println!("mode switched to {target}");
            Ok(())
        }
        _ => unreachable!("application routes only control commands here"),
    }
}
