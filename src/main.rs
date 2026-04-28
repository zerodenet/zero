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

    #[cfg(not(feature = "status-api"))]
    ensure_status_api_not_configured(&engine, status_listen)?;

    let event_dispatcher = spawn_event_dispatcher_if_configured(&engine)?;

    tracing::info!(config = %config_path, "loaded engine configuration");

    #[cfg(feature = "status-api")]
    {
        if let Some(status) = status_server_spec(&engine, status_listen)? {
            let probe = engine.clone();
            let status_server =
                status_server::spawn_status_server(probe, &status.listen, status.auth).await?;
            let running = engine.spawn();

            wait_for_shutdown_signal().await;

            shutdown_event_dispatcher(event_dispatcher).await;
            status_server.shutdown().await?;
            running.shutdown().await?;

            return Ok(());
        }
    }

    if event_dispatcher.is_some() {
        let running = engine.spawn();
        wait_for_shutdown_signal().await;

        shutdown_event_dispatcher(event_dispatcher).await;
        running.shutdown().await?;
    } else {
        engine.run().await?;
    }

    Ok(())
}

#[cfg(feature = "status-api")]
struct StatusServerSpec {
    listen: String,
    auth: Option<status_server::StatusServerAuth>,
}

#[cfg(feature = "status-api")]
fn status_server_spec(
    engine: &Engine,
    cli_listen: Option<&str>,
) -> Result<Option<StatusServerSpec>, Box<dyn Error>> {
    let control = &engine.config().api.control;

    if cli_listen.is_some() && control.enabled {
        return Err(std::io::Error::other(
            "use either `--status-listen` or `api.control`, not both",
        )
        .into());
    }

    if let Some(listen) = cli_listen {
        return Ok(Some(StatusServerSpec {
            listen: listen.to_owned(),
            auth: None,
        }));
    }

    if !control.enabled {
        return Ok(None);
    }

    let listen = control
        .listen
        .as_ref()
        .expect("config validation requires api.control.listen");
    let key = config_api_key(control.api_key.as_ref(), control.api_key_env.as_ref())?;

    Ok(Some(StatusServerSpec {
        listen: format!("{}:{}", listen.address, listen.port),
        auth: Some(status_server::StatusServerAuth::new(key)),
    }))
}

#[cfg(not(feature = "status-api"))]
fn ensure_status_api_not_configured(
    engine: &Engine,
    cli_listen: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    if let Some(status_listen) = cli_listen {
        return Err(std::io::Error::other(format!(
            "`--status-listen {status_listen}` requires Cargo feature `status-api`"
        ))
        .into());
    }

    if engine.config().api.control.enabled {
        return Err(std::io::Error::other(
            "`api.control.enabled` requires Cargo feature `status-api`",
        )
        .into());
    }

    Ok(())
}

#[cfg(feature = "status-api")]
fn config_api_key(
    api_key: Option<&String>,
    api_key_env: Option<&String>,
) -> Result<String, Box<dyn Error>> {
    if let Some(key) = api_key {
        return Ok(key.clone());
    }

    let name = api_key_env.expect("config validation requires api_key or api_key_env");
    let value = env::var(name)?;
    if value.trim().is_empty() {
        return Err(std::io::Error::other(format!(
            "api key environment variable `{name}` must not be empty"
        ))
        .into());
    }
    Ok(value)
}

async fn wait_for_shutdown_signal() {
    match tokio::signal::ctrl_c().await {
        Ok(()) => tracing::info!("shutdown signal received"),
        Err(error) => {
            tracing::warn!(error = %error, "failed to listen for ctrl-c; stopping engine")
        }
    }
}

#[cfg(feature = "event-dispatcher")]
fn spawn_event_dispatcher_if_configured(
    engine: &Engine,
) -> Result<Option<zero_connector::EventDispatcherHandle>, Box<dyn Error>> {
    let config = engine.config();
    let dispatcher = zero_connector::spawn_event_dispatcher(
        engine.clone(),
        config.api.clone(),
        config.source_dir.clone(),
        zero_connector::EventDispatcherOptions::default(),
    )?;
    Ok(dispatcher)
}

#[cfg(not(feature = "event-dispatcher"))]
fn spawn_event_dispatcher_if_configured(
    engine: &Engine,
) -> Result<Option<EventDispatcherUnavailable>, Box<dyn Error>> {
    if engine.config().api.event_sinks.is_empty() {
        return Ok(None);
    }

    Err(std::io::Error::other("`api.event_sinks` requires Cargo feature `event-dispatcher`").into())
}

#[cfg(feature = "event-dispatcher")]
async fn shutdown_event_dispatcher(dispatcher: Option<zero_connector::EventDispatcherHandle>) {
    if let Some(dispatcher) = dispatcher {
        dispatcher.shutdown().await;
    }
}

#[cfg(not(feature = "event-dispatcher"))]
async fn shutdown_event_dispatcher(_dispatcher: Option<EventDispatcherUnavailable>) {}

#[cfg(not(feature = "event-dispatcher"))]
struct EventDispatcherUnavailable;

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
