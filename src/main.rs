use std::env;
use std::error::Error;
use std::process;

use tracing_subscriber::EnvFilter;
use zero_engine::Engine;

mod cli;
mod error_report;
mod output;
#[cfg(feature = "status-api")]
mod status_server;

#[tokio::main]
async fn main() {
    init_tracing();

    if let Err(error) = try_main().await {
        error_report::print_error(error.as_ref());
        process::exit(1);
    }
}

async fn try_main() -> Result<(), Box<dyn Error>> {
    match cli::parse_args(env::args())? {
        cli::Command::Run {
            config_path,
            status_listen,
        } => run_command(&config_path, status_listen.as_deref()).await?,
        cli::Command::Status { config_path, json } => status_command(&config_path, json)?,
        cli::Command::Help => println!("{}", cli::usage()),
    }

    Ok(())
}

async fn run_command(config_path: &str, status_listen: Option<&str>) -> Result<(), Box<dyn Error>> {
    let engine = Engine::from_path(config_path)?;

    tracing::info!(config = %config_path, "loaded engine configuration");

    if let Some(status_listen) = status_listen {
        #[cfg(feature = "status-api")]
        {
            let probe = engine.clone();
            let status_server = status_server::spawn_status_server(probe, status_listen).await?;
            let running = engine.spawn();

            match tokio::signal::ctrl_c().await {
                Ok(()) => tracing::info!("shutdown signal received"),
                Err(error) => {
                    tracing::warn!(error = %error, "failed to listen for ctrl-c; stopping engine")
                }
            }

            status_server.shutdown().await?;
            running.shutdown().await?;
        }
        #[cfg(not(feature = "status-api"))]
        {
            return Err(std::io::Error::other(format!(
                "`--status-listen {status_listen}` requires Cargo feature `status-api`"
            ))
            .into());
        }
    } else {
        engine.run().await?;
    }

    Ok(())
}

fn status_command(config_path: &str, json: bool) -> Result<(), Box<dyn Error>> {
    let engine = Engine::from_path(config_path)?;
    let status = engine.export_status();

    if json {
        println!("{}", serde_json::to_string_pretty(&status)?);
    } else {
        print!("{}", output::render_status(&status));
    }

    Ok(())
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .expect("valid tracing filter");

    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .compact()
        .try_init();
}
