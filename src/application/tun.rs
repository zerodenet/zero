use std::error::Error;

use crate::cli::Command;
use crate::ipc::protocol::IpcRequest;

pub fn execute(command: Command) -> Result<(), Box<dyn Error>> {
    match command {
        Command::TunStart {
            name,
            addr,
            mask,
            mtu,
            tag,
            socket_path,
        } => {
            let request = IpcRequest::Command {
                id: None,
                method: "tun.start".to_owned(),
                params: serde_json::json!({
                    "name": name,
                    "addr": addr,
                    "mask": mask.unwrap_or_else(|| "255.255.255.0".to_owned()),
                    "mtu": mtu.unwrap_or(1500),
                    "tag": tag,
                }),
            };
            send_command(socket_path.as_deref(), request, "tun started")
        }
        Command::TunStop { socket_path } => send_command(
            socket_path.as_deref(),
            IpcRequest::Command {
                id: None,
                method: "tun.stop".to_owned(),
                params: serde_json::json!({}),
            },
            "tun stopped",
        ),
        Command::TunStatus { socket_path } => {
            let socket = super::resolve_socket(socket_path.as_deref())?;
            let response = crate::ipc::client::send_request(
                &socket,
                &IpcRequest::Query {
                    id: None,
                    request: zero_api::QueryRequest::TunStatus(zero_api::TunStatusQuery),
                },
            )?;
            if !response.ok {
                return Err(response
                    .error
                    .map(|error| error.message)
                    .unwrap_or_default()
                    .into());
            }
            let status: zero_api::TunStatusSnapshot =
                serde_json::from_value(response.result.unwrap_or_default())?;
            if status.running {
                println!(
                    "tun: running, name={}, addr={}, tag={}",
                    status.name.as_deref().unwrap_or("-"),
                    status.addr.as_deref().unwrap_or("-"),
                    status.tag.as_deref().unwrap_or("-")
                );
            } else {
                println!("tun: not running");
            }
            Ok(())
        }
        _ => unreachable!("application routes only tun commands here"),
    }
}

fn send_command(
    socket_path: Option<&str>,
    request: IpcRequest,
    success: &str,
) -> Result<(), Box<dyn Error>> {
    let socket = super::resolve_socket(socket_path)?;
    let response = crate::ipc::client::send_request(&socket, &request)?;
    if response.ok {
        println!("{success}");
        Ok(())
    } else {
        Err(response
            .error
            .map(|error| error.message)
            .unwrap_or_default()
            .into())
    }
}
