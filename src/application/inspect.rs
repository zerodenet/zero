use std::error::Error;

use crate::cli::Command;

pub fn execute(command: Command) -> Result<(), Box<dyn Error>> {
    match command {
        Command::Status {
            config_path,
            json,
            socket_path,
        } => crate::status_command(config_path.as_deref(), json, socket_path.as_deref()),
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
