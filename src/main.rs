use std::env;
use std::error::Error;
use std::process;

use tracing_subscriber::EnvFilter;
use zero_api::{
    CommandRequest, FlowListQuery, PoliciesQuery, PolicySelectCommand, QueryRequest, QueryService,
};
use zero_engine::{Engine, EngineHandle};
use zero_proxy::Proxy;

mod cli;
mod error_report;
#[cfg(feature = "status-api")]
mod http_adapter;
mod hooks;
mod ipc;
mod output;

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
            control_socket,
            ipc_hook_socket,
        } => run_command(
            &config_path,
            status_listen.as_deref(),
            control_socket.as_deref(),
            ipc_hook_socket.as_deref(),
        )
        .await?,
        cli::Command::Status {
            config_path,
            json,
            socket_path,
        } => status_command(config_path.as_deref(), json, socket_path.as_deref())?,
        cli::Command::Select {
            policy_tag,
            target_tag,
            socket_path,
        } => select_command(&policy_tag, &target_tag, socket_path.as_deref())?,
        cli::Command::Flows { socket_path } => flows_command(socket_path.as_deref())?,
        cli::Command::Policies { socket_path } => policies_command(socket_path.as_deref())?,
        cli::Command::Events { socket_path } => events_command(socket_path.as_deref())?,
        cli::Command::Reload {
            config_path,
            socket_path,
        } => reload_command(&config_path, socket_path.as_deref())?,
        cli::Command::Help => println!("{}", cli::usage()),
    }

    Ok(())
}

async fn run_command(
    config_path: &str,
    status_listen: Option<&str>,
    control_socket: Option<&str>,
    ipc_hook_socket: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    let proxy = Proxy::from_path(config_path)?;
    let engine = proxy.engine().clone();

    // Build hook chain from config/cli options.
    let warning_handler = {
        let engine = engine.clone();
        Some(std::sync::Arc::new(move |code: &str, msg: &str| {
            engine.emit_warning(code, msg);
        }) as std::sync::Arc<dyn Fn(&str, &str) + Send + Sync>)
    };
    let hook_chain =
        hooks::build_hook_chain(ipc_hook_socket, &engine.config().api, warning_handler);
    let engine = if !hook_chain.is_empty() {
        engine.with_flow_hook_chain(hook_chain)
    } else {
        engine
    };

    let engine_handle = EngineHandle::new(engine.clone());

    #[cfg(not(feature = "status-api"))]
    ensure_status_api_not_configured(&engine, status_listen)?;

    let event_dispatcher = spawn_event_dispatcher_if_configured(&engine)?;

    tracing::info!(config = %config_path, "loaded proxy configuration");

    // IPC server always starts (not feature-gated).
    let ipc_socket_path = ipc::resolve_ipc_path(control_socket)?;
    let ipc_server =
        ipc::spawn_ipc_server(engine_handle.clone(), &ipc_socket_path).await?;

    #[cfg(feature = "status-api")]
    let http_server = {
        if let Some(status) = status_server_spec(&engine, status_listen)? {
            Some(
                http_adapter::spawn_http_server(
                    engine_handle,
                    &status.listen,
                    status.auth,
                )
                .await?,
            )
        } else {
            None
        }
    };

    let running = proxy.spawn();
    let stats_sampler = spawn_stats_sampler(engine.clone());
    let push_connector = spawn_push_connector_if_configured(&engine)?;

    wait_for_shutdown_signal().await;

    stats_sampler.abort();
    shutdown_push_connector(push_connector).await;

    shutdown_event_dispatcher(event_dispatcher).await;
    ipc_server.shutdown().await?;
    #[cfg(feature = "status-api")]
    if let Some(s) = http_server {
        s.shutdown().await?;
    }
    running.shutdown().await?;

    Ok(())
}

#[cfg(feature = "status-api")]
struct StatusServerSpec {
    listen: String,
    auth: Option<http_adapter::HttpServerAuth>,
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
        auth: Some(http_adapter::HttpServerAuth::new(key)),
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
            tracing::warn!(error = %error, "failed to listen for ctrl-c; stopping proxy")
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

    Err(
        std::io::Error::other(
            "`api.event_sinks` requires Cargo feature `event-dispatcher`",
        )
        .into(),
    )
}

#[cfg(feature = "event-dispatcher")]
async fn shutdown_event_dispatcher(dispatcher: Option<zero_connector::EventDispatcherHandle>) {
    if let Some(dispatcher) = dispatcher {
        dispatcher.shutdown().await;
    }
}

#[cfg(not(feature = "event-dispatcher"))]
async fn shutdown_event_dispatcher(_dispatcher: Option<EventDispatcherUnavailable>) {}

#[cfg(feature = "panel-connector")]
fn spawn_push_connector_if_configured(
    engine: &Engine,
) -> Result<Option<zero_connector::PushConnectorHandle>, Box<dyn Error>> {
    let config = &engine.config().push;
    if !config.enabled() {
        return Ok(None);
    }

    let engine_clone = engine.clone();
    let version = env!("CARGO_PKG_VERSION").to_owned();

    let handle = zero_connector::spawn_push_connector(
        config,
        engine_clone,
        {
            let engine = engine.clone();
            move || {
                use std::time::{SystemTime, UNIX_EPOCH};
                let now_ms = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                // uptime is approximated from the events log; use a simple counter.
                // The engine doesn't store start time, so we track it here.
                static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
                let start = START.get_or_init(std::time::Instant::now);
                start.elapsed().as_secs()
            }
        },
        {
            let engine = engine.clone();
            move || engine.stats_snapshot().active_sessions as usize
        },
        {
            let engine = engine.clone();
            move || engine.stats_snapshot().bytes_up
        },
        {
            let engine = engine.clone();
            move || engine.stats_snapshot().bytes_down
        },
        &version,
    )?;

    Ok(handle)
}

#[cfg(not(feature = "panel-connector"))]
fn spawn_push_connector_if_configured(
    engine: &Engine,
) -> Result<Option<PushConnectorUnavailable>, Box<dyn Error>> {
    if engine.config().push.enabled() {
        return Err(std::io::Error::other(
            "`push` requires Cargo feature `panel-connector`",
        )
        .into());
    }
    Ok(None)
}

#[cfg(feature = "panel-connector")]
async fn shutdown_push_connector(
    connector: Option<zero_connector::PushConnectorHandle>,
) {
    if let Some(c) = connector {
        c.shutdown().await;
    }
}

#[cfg(not(feature = "panel-connector"))]
async fn shutdown_push_connector(
    _connector: Option<PushConnectorUnavailable>,
) {}

#[cfg(not(feature = "panel-connector"))]
struct PushConnectorUnavailable;

#[cfg(not(feature = "event-dispatcher"))]
struct EventDispatcherUnavailable;

// ── IPC client commands ───────────────────────────────────────────────

fn resolve_socket(socket_path: Option<&str>) -> Result<String, Box<dyn Error>> {
    if let Some(path) = socket_path {
        return Ok(path.to_owned());
    }
    let path = ipc::default_ipc_path().ok_or_else(|| {
        std::io::Error::other(
            "cannot find control socket: $HOME is not set. Use --socket to specify a path.",
        )
    })?;
    if !path.exists() {
        return Err(std::io::Error::other(format!(
            "control socket not found at {} — is zero running?",
            path.display()
        ))
        .into());
    }
    Ok(path.to_string_lossy().to_string())
}

fn status_command(
    config_path: Option<&str>,
    json: bool,
    socket_path: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    // Try IPC first if no config path (connect to running daemon).
    if config_path.is_none() {
        if let Ok(socket) = resolve_socket(socket_path) {
            let request = crate::ipc::protocol::IpcRequest::Query {
                request: QueryRequest::Runtime(Default::default()),
            };
            let response = ipc::client::send_request(&socket, &request)?;
            let result = response.result.unwrap_or_default();
            if json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                // Print a compact human-readable summary from the JSON blob.
                let runtime = &result;
                println!("Engine Status (via {socket})");
                if let Some(stats) = runtime.get("stats") {
                    println!(
                        "  sessions: active={} total={} completed={} failed={}",
                        stats["active_sessions"].as_u64().unwrap_or(0),
                        stats["total_started"].as_u64().unwrap_or(0),
                        stats["completed_sessions"].as_u64().unwrap_or(0),
                        stats["failed_sessions"].as_u64().unwrap_or(0),
                    );
                }
                if let Some(inbounds) = runtime.get("active_sessions").and_then(|v| v.as_array()) {
                    println!("  active_flows: {}", inbounds.len());
                }
                if let Some(completed) = runtime
                    .get("recent_completed_sessions")
                    .and_then(|v| v.as_array())
                {
                    println!("  recent_completed: {}", completed.len());
                }
            }
            return Ok(());
        }
    }

    // Fallback: offline mode, read config directly.
    let path = config_path.unwrap_or(cli::DEFAULT_CONFIG_PATH);
    let proxy = Proxy::from_path(path)?;
    let status = proxy.export_status();

    if json {
        println!("{}", serde_json::to_string_pretty(&status)?);
    } else {
        print!("{}", output::render_status(&status));
    }

    Ok(())
}

fn select_command(
    policy_tag: &str,
    target_tag: &str,
    socket_path: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    let socket = resolve_socket(socket_path)?;
    let request = crate::ipc::protocol::IpcRequest::Command {
        method: "policies.select".to_owned(),
        params: serde_json::json!({
            "policy_tag": policy_tag,
            "target_tag": target_tag,
        }),
    };
    let response = ipc::client::send_request(&socket, &request)?;
    if response.ok {
        println!("selector `{policy_tag}` → `{target_tag}`");
    } else {
        let msg = response
            .error
            .as_ref()
            .map(|e| e.message.as_str())
            .unwrap_or("unknown error");
        eprintln!("error: {msg}");
        process::exit(1);
    }
    Ok(())
}

fn flows_command(socket_path: Option<&str>) -> Result<(), Box<dyn Error>> {
    let socket = resolve_socket(socket_path)?;
    let request = crate::ipc::protocol::IpcRequest::Query {
        request: QueryRequest::ActiveFlows(FlowListQuery {
            limit: Some(100),
            filter: Default::default(),
        }),
    };
    let response = ipc::client::send_request(&socket, &request)?;
    if response.ok {
        if let Some(result) = response.result {
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
    } else {
        let msg = response
            .error
            .as_ref()
            .map(|e| e.message.as_str())
            .unwrap_or("unknown error");
        eprintln!("error: {msg}");
        process::exit(1);
    }
    Ok(())
}

fn policies_command(socket_path: Option<&str>) -> Result<(), Box<dyn Error>> {
    let socket = resolve_socket(socket_path)?;
    let request = crate::ipc::protocol::IpcRequest::Query {
        request: QueryRequest::Policies(PoliciesQuery),
    };
    let response = ipc::client::send_request(&socket, &request)?;
    if response.ok {
        if let Some(result) = response.result {
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
    } else {
        let msg = response
            .error
            .as_ref()
            .map(|e| e.message.as_str())
            .unwrap_or("unknown error");
        eprintln!("error: {msg}");
        process::exit(1);
    }
    Ok(())
}

fn events_command(socket_path: Option<&str>) -> Result<(), Box<dyn Error>> {
    let socket = resolve_socket(socket_path)?;
    let request = crate::ipc::protocol::IpcRequest::Subscribe { events: None };
    ipc::client::stream_events(&socket, &request, |event| {
        println!("{}", serde_json::to_string(&event).unwrap_or_default());
    })?;
    Ok(())
}

fn spawn_stats_sampler(engine: Engine) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut stats_tick = tokio::time::interval(std::time::Duration::from_secs(30));
        let mut flow_tick = tokio::time::interval(std::time::Duration::from_secs(10));
        loop {
            tokio::select! {
                _ = stats_tick.tick() => engine.push_stats_sampled(),
                _ = flow_tick.tick() => engine.push_flow_updates(),
            }
        }
    })
}

fn reload_command(
    config_path: &str,
    socket_path: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    let socket = resolve_socket(socket_path)?;
    let config_str = std::fs::read_to_string(config_path)?;
    let config_value: serde_json::Value =
        serde_json::from_str(&config_str).map_err(std::io::Error::other)?;

    let request = crate::ipc::protocol::IpcRequest::Command {
        method: "config.apply".to_owned(),
        params: config_value,
    };
    let response = ipc::client::send_request(&socket, &request)?;
    if response.ok {
        println!("config applied: route rules hot-reloaded");
    } else {
        let msg = response
            .error
            .as_ref()
            .map(|e| e.message.as_str())
            .unwrap_or("unknown error");
        eprintln!("error: {msg}");
        process::exit(1);
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
