use std::error::Error;

use crate::cli::Command;
use std::env;
use zero_engine::{Engine, EngineHandle};
use zero_proxy::{Proxy, ProxyHandle};

#[cfg(feature = "status_api")]
use crate::http_adapter;
use crate::{hooks, ipc, rule_set_fetch};

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
    run(
        &config_path,
        status_listen.as_deref(),
        control_socket.as_deref(),
        ipc_hook_socket.as_deref(),
    )
    .await
}

async fn run(
    config_path: &str,
    status_listen: Option<&str>,
    control_socket: Option<&str>,
    ipc_hook_socket: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    let mut config = zero_config::RuntimeConfig::load_from_path(config_path)?;
    rule_set_fetch::pre_fetch_rule_sets(&mut config.route.rule_sets, config.source_dir.as_deref());
    let proxy = Proxy::from_engine(zero_engine::Engine::new(config)?)?;
    let engine = proxy.engine().clone();

    // Build hook chain from config/cli options.
    let warning_handler = {
        let engine = engine.clone();
        Some(std::sync::Arc::new(move |code: &str, msg: &str| {
            engine.emit_warning(code, msg);
        })
            as std::sync::Arc<dyn Fn(&str, &str) + Send + Sync>)
    };
    let hook_chain =
        hooks::build_hook_chain(ipc_hook_socket, &engine.config().api, warning_handler);
    let engine = if !hook_chain.is_empty() {
        engine.with_flow_hook_chain(hook_chain)
    } else {
        engine
    };

    let engine_handle = EngineHandle::new(engine.clone());
    let ipc_handle = ProxyHandle::new(engine_handle.clone(), proxy.clone());

    // Bridge tracing warn/error ->?engine.warning events.
    {
        let e = engine.clone();
        zero_logging::set_warning_sink(move |code: &str, msg: &str| {
            e.emit_warning(code, msg);
        });
    }

    #[cfg(not(any(feature = "status_api", feature = "grpc_api")))]
    ensure_status_api_not_configured(&engine, status_listen)?;

    let event_dispatcher = spawn_event_dispatcher_if_configured(&engine)?;

    // Bridge dispatcher sink status into Engine so /api/v1/sinks is live.
    #[cfg(feature = "event_dispatcher")]
    if let Some(ref dispatcher) = event_dispatcher {
        engine.update_sink_status(dispatcher.sink_status());
        // Periodically refresh sink status in the background.
        let engine_ref = engine.clone();
        let dispatcher_status = dispatcher.status_handle();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
            loop {
                interval.tick().await;
                engine_ref.update_sink_status(dispatcher_status.sink_status());
            }
        });
    }

    tracing::info!(config = %config_path, "loaded proxy configuration");

    // IPC server always starts (not feature-gated).
    let ipc_socket_path = ipc::resolve_ipc_path(control_socket)?;
    let ipc_server = ipc::spawn_ipc_server(ipc_handle.clone(), &ipc_socket_path).await?;

    #[cfg(any(feature = "status_api", feature = "grpc_api"))]
    let status_spec = status_server_spec(&engine, status_listen)?;

    #[cfg(feature = "status_api")]
    let http_server = {
        if let Some(ref status) = status_spec {
            Some(
                http_adapter::spawn_http_server(
                    ipc_handle.clone(),
                    &status.listen,
                    status.auth.clone(),
                )
                .await?,
            )
        } else {
            None
        }
    };

    #[cfg(feature = "grpc_api")]
    let grpc_server = {
        if let Some(ref status) = status_spec {
            let addr: std::net::SocketAddr = status
                .grpc_listen
                .parse()
                .map_err(|e| std::io::Error::other(format!("gRPC listen address: {e}")))?;
            Some(zero_grpc::spawn(engine_handle.clone(), addr).await?)
        } else {
            None
        }
    };

    let running = proxy.spawn();
    let stats_sampler = spawn_stats_sampler(engine.clone());
    let push_connector = spawn_push_connector_if_configured(&engine)?;

    wait_for_shutdown_signal().await;

    engine.push_engine_stopped("signal");
    // Allow the event dispatcher a brief window to flush the event.
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    stats_sampler.abort();
    shutdown_push_connector(push_connector).await;

    shutdown_event_dispatcher(event_dispatcher).await;
    ipc_server.shutdown().await?;
    #[cfg(feature = "status_api")]
    if let Some(s) = http_server {
        s.shutdown().await?;
    }
    #[cfg(feature = "grpc_api")]
    if let Some(s) = grpc_server {
        s.shutdown().await;
    }
    running.shutdown().await?;

    Ok(())
}

#[cfg(any(feature = "status_api", feature = "grpc_api"))]
struct StatusServerSpec {
    listen: String,
    #[cfg(feature = "grpc_api")]
    grpc_listen: String,
    #[cfg(feature = "status_api")]
    auth: Option<http_adapter::HttpServerAuth>,
}

#[cfg(any(feature = "status_api", feature = "grpc_api"))]
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
            #[cfg(feature = "grpc_api")]
            grpc_listen: next_port(listen),
            #[cfg(feature = "status_api")]
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

    #[cfg(feature = "status_api")]
    let auth = {
        let key = config_api_key(control.api_key.as_ref(), control.api_key_env.as_ref())?;
        Some(http_adapter::HttpServerAuth::single_admin(key))
    };

    Ok(Some(StatusServerSpec {
        listen: format!("{}:{}", listen.address, listen.port),
        #[cfg(feature = "grpc_api")]
        grpc_listen: format!("{}:{}", listen.address, listen.port + 1),
        #[cfg(feature = "status_api")]
        auth,
    }))
}

#[cfg(feature = "grpc_api")]
fn next_port(listen: &str) -> String {
    if let Some(idx) = listen.rfind(':') {
        let (host, port_str) = listen.split_at(idx + 1);
        if let Ok(port) = port_str.parse::<u16>() {
            return format!("{host}{}", port + 1);
        }
    }
    // fallback: append :9091
    format!("{listen}:9091")
}

#[cfg(not(any(feature = "status_api", feature = "grpc_api")))]
fn ensure_status_api_not_configured(
    engine: &Engine,
    cli_listen: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    if let Some(status_listen) = cli_listen {
        return Err(std::io::Error::other(format!(
            "`--status-listen {status_listen}` requires Cargo feature `status_api`"
        ))
        .into());
    }

    if engine.config().api.control.enabled {
        return Err(std::io::Error::other(
            "`api.control.enabled` requires Cargo feature `status_api`",
        )
        .into());
    }

    Ok(())
}

#[cfg(feature = "status_api")]
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

fn spawn_stats_sampler(engine: Engine) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut stats_tick = tokio::time::interval(std::time::Duration::from_secs(1));
        let mut flow_tick = tokio::time::interval(std::time::Duration::from_secs(1));
        loop {
            tokio::select! {
                _ = stats_tick.tick() => engine.push_stats_sampled(),
                _ = flow_tick.tick() => engine.push_flow_updates(),
            }
        }
    })
}

#[cfg(feature = "event_dispatcher")]
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

#[cfg(not(feature = "event_dispatcher"))]
fn spawn_event_dispatcher_if_configured(
    engine: &Engine,
) -> Result<Option<EventDispatcherUnavailable>, Box<dyn Error>> {
    if engine.config().api.event_sinks.is_empty() {
        return Ok(None);
    }

    Err(std::io::Error::other("`api.event_sinks` requires Cargo feature `event_dispatcher`").into())
}

#[cfg(feature = "event_dispatcher")]
async fn shutdown_event_dispatcher(dispatcher: Option<zero_connector::EventDispatcherHandle>) {
    if let Some(dispatcher) = dispatcher {
        dispatcher.shutdown().await;
    }
}

#[cfg(not(feature = "event_dispatcher"))]
async fn shutdown_event_dispatcher(_dispatcher: Option<EventDispatcherUnavailable>) {}

#[cfg(feature = "panel_connector")]
fn spawn_push_connector_if_configured(
    engine: &Engine,
) -> Result<Option<zero_connector::PushConnectorHandle>, Box<dyn Error>> {
    let config = &engine.config().push;
    if !config.enabled() {
        return Ok(None);
    }

    let engine_clone = engine.clone();
    let build_id = env!("CARGO_PKG_VERSION").to_owned();

    let handle = zero_connector::spawn_push_connector(
        config,
        engine_clone,
        {
            move || {
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
        &build_id,
    )?;

    Ok(handle)
}

#[cfg(not(feature = "panel_connector"))]
fn spawn_push_connector_if_configured(
    engine: &Engine,
) -> Result<Option<PushConnectorUnavailable>, Box<dyn Error>> {
    if engine.config().push.enabled() {
        return Err(
            std::io::Error::other("`push` requires Cargo feature `panel_connector`").into(),
        );
    }
    Ok(None)
}

#[cfg(feature = "panel_connector")]
async fn shutdown_push_connector(connector: Option<zero_connector::PushConnectorHandle>) {
    if let Some(c) = connector {
        c.shutdown().await;
    }
}

#[cfg(not(feature = "panel_connector"))]
async fn shutdown_push_connector(_connector: Option<PushConnectorUnavailable>) {}

#[cfg(not(feature = "panel_connector"))]
struct PushConnectorUnavailable;

#[cfg(not(feature = "event_dispatcher"))]
struct EventDispatcherUnavailable;

// ── IPC client commands ───────────────────────────────────────────────
