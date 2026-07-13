use std::error::Error;

use zero_api::QueryRequest;

use super::resolve_socket;
use crate::cli::Command;

pub fn execute(command: Command) -> Result<(), Box<dyn Error>> {
    match command {
        Command::Status {
            config_path,
            json,
            socket_path,
        } => status(config_path.as_deref(), json, socket_path.as_deref()),
        Command::Validate { config_path } => {
            let config = zero_config::RuntimeConfig::load_from_path(&config_path)?;
            let proxy = zero_proxy::Proxy::from_engine(zero_engine::Engine::new(config)?)?;
            println!(
                "config valid: {} inbounds, {} outbounds, {} groups, {} rules",
                proxy.config().inbounds.len(),
                proxy.config().outbounds.len(),
                proxy.config().outbound_groups.len(),
                proxy.config().route.rules.len(),
            );
            Ok(())
        }
        Command::BuildInfo => {
            println!("build_id: {}", env!("CARGO_PKG_VERSION"));
            println!("build_time: {}", env!("ZERO_BUILD_TIME"));
            if let Some(hash) = option_env!("ZERO_GIT_DESCRIBE").or(option_env!("ZERO_GIT_HASH")) {
                println!("git: {hash}");
            }
            Ok(())
        }
        Command::Help => {
            println!("{}", crate::cli::usage());
            Ok(())
        }
        _ => unreachable!("application routes only inspect commands here"),
    }
}

fn status(
    config_path: Option<&str>,
    json: bool,
    socket_path: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    if config_path.is_none() {
        if let Ok(socket) = resolve_socket(socket_path) {
            let response = crate::ipc::client::send_request(
                &socket,
                &crate::ipc::protocol::IpcRequest::Query {
                    id: None,
                    request: QueryRequest::Runtime(Default::default()),
                },
            )?;
            let result = response.result.unwrap_or_default();
            if json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("Engine Status (via {socket})");
                if let Some(stats) = result.get("stats") {
                    println!(
                        "  sessions: active={} total={} completed={} failed={}",
                        stats["active_sessions"].as_u64().unwrap_or(0),
                        stats["total_started"].as_u64().unwrap_or(0),
                        stats["completed_sessions"].as_u64().unwrap_or(0),
                        stats["failed_sessions"].as_u64().unwrap_or(0),
                    );
                }
                println!(
                    "  active_flows: {}",
                    result
                        .get("active_sessions")
                        .and_then(|value| value.as_array())
                        .map_or(0, Vec::len)
                );
                println!(
                    "  recent_completed: {}",
                    result
                        .get("recent_completed_sessions")
                        .and_then(|value| value.as_array())
                        .map_or(0, Vec::len)
                );
            }
            return Ok(());
        }
    }

    let path = config_path
        .ok_or_else(|| std::io::Error::other("no config path provided and no socket available"))?;
    let status = zero_proxy::Proxy::from_path(path)?.export_status();
    if json {
        println!("{}", serde_json::to_string_pretty(&status)?);
    } else {
        print!("{}", crate::output::render_status(&status));
    }
    Ok(())
}
