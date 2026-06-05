use std::time::Duration;

use serde::Serialize;
use tokio::sync::oneshot;
use tracing::{debug, info, warn};
use zero_api::{
    CommandRequest, CommandService, DiagnosticsDnsLookupCommand, DiagnosticsProbeTargetCommand,
    DiagnosticsTraceRouteCommand, FlowCloseCommand, ModeSetCommand, PolicyProbeCommand,
    PolicySelectCommand, TunStartCommand, TunStopCommand,
};
use zero_config::PushConfig;

use crate::{ConnectorError, ConnectorResult};

/// A connector that actively pushes heartbeats to a central panel and
/// optionally polls for pending commands.
///
/// # Protocol (HTTP JSON)
///
/// **Heartbeat** — POST `/api/v1/nodes/{node_id}/heartbeat`
/// ```json
/// {"node_id":"node-001","build_id":"0.0.2","uptime_seconds":3600,"active_flows":42,...}
/// ```
/// Response: `{"ok":true}` or `{"ok":true,"commands":[{"method":"policies.select",...}]}`
///
/// **Command result** — POST `/api/v1/nodes/{node_id}/commands/{cmd_id}/result`
/// ```json
/// {"command_id":"cmd-1","ok":true,"result":{...}}
/// ```
pub struct PushConnectorHandle {
    shutdown: Option<oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<()>,
}

impl PushConnectorHandle {
    pub async fn shutdown(mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        let _ = self.task.await;
    }
}

/// Spawn the panel connector if the panel config is enabled.
///
/// Returns `None` when `config.enabled()` is false.
pub fn spawn_push_connector<C>(
    config: &PushConfig,
    command_service: C,
    uptime_seconds: impl Fn() -> u64 + Send + Sync + 'static,
    active_flows: impl Fn() -> usize + Send + Sync + 'static,
    bytes_up: impl Fn() -> u64 + Send + Sync + 'static,
    bytes_down: impl Fn() -> u64 + Send + Sync + 'static,
    build_id: &str,
) -> ConnectorResult<Option<PushConnectorHandle>>
where
    C: CommandService + Clone + Send + Sync + 'static,
{
    if !config.enabled() {
        return Ok(None);
    }

    let url = config
        .url
        .as_deref()
        .expect("panel url required")
        .to_owned();
    let node_id = config
        .node_id
        .as_deref()
        .expect("panel node_id required")
        .to_owned();
    let api_key = resolve_api_key(&config.api_key, &config.api_key_env)?;
    let heartbeat_interval = Duration::from_secs(config.heartbeat_interval_seconds);
    let pull_commands = config.pull_commands;
    let command_poll_interval = Duration::from_secs(config.command_poll_interval_seconds);
    let build_id = build_id.to_owned();

    let info_url = url.clone();
    let info_node_id = node_id.clone();
    let info_interval = heartbeat_interval.as_secs();

    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    let task = tokio::spawn(async move {
        run_panel_connector(
            url,
            node_id,
            api_key,
            heartbeat_interval,
            pull_commands,
            command_poll_interval,
            build_id,
            command_service,
            uptime_seconds,
            active_flows,
            bytes_up,
            bytes_down,
            shutdown_rx,
        )
        .await;
    });

    info!(
        url = %info_url,
        node_id = %info_node_id,
        heartbeat_interval_seconds = info_interval,
        "panel connector started"
    );

    Ok(Some(PushConnectorHandle {
        shutdown: Some(shutdown_tx),
        task,
    }))
}

async fn run_panel_connector<C>(
    url: String,
    node_id: String,
    api_key: String,
    heartbeat_interval: Duration,
    pull_commands: bool,
    command_poll_interval: Duration,
    build_id: String,
    command_service: C,
    uptime_seconds: impl Fn() -> u64,
    active_flows: impl Fn() -> usize,
    bytes_up: impl Fn() -> u64,
    bytes_down: impl Fn() -> u64,
    mut shutdown: oneshot::Receiver<()>,
) where
    C: CommandService + Clone,
{
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            warn!(error = %e, "failed to build panel http client");
            return;
        }
    };

    let heartbeat_url = format!("{url}/api/v1/nodes/{node_id}/heartbeat");
    let commands_url = format!("{url}/api/v1/nodes/{node_id}/commands");

    let mut tick = tokio::time::interval(heartbeat_interval);
    let mut cmd_tick = tokio::time::interval(command_poll_interval);

    // Keepalive tracking.
    let mut last_success = std::time::Instant::now();
    let mut consecutive_failures: u32 = 0;
    const MAX_BACKOFF_SECS: u64 = 64;

    loop {
        tokio::select! {
            _ = &mut shutdown => break,
            _ = tick.tick() => {
                let hb = Heartbeat {
                    node_id: node_id.clone(),
                    build_id: build_id.clone(),
                    uptime_seconds: uptime_seconds(),
                    active_flows: active_flows(),
                    bytes_up: bytes_up(),
                    bytes_down: bytes_down(),
                };

                match send_heartbeat(&client, &heartbeat_url, &api_key, &hb).await {
                    Ok(resp) => {
                        last_success = std::time::Instant::now();
                        consecutive_failures = 0;
                        debug!(node_id = %node_id, "heartbeat sent");

                        if let Some(commands) = resp.get("commands").and_then(|c| c.as_array()) {
                            for cmd in commands {
                                execute_panel_command(&command_service, cmd).await;
                            }
                        }
                    }
                    Err(e) => {
                        consecutive_failures += 1;
                        let backoff_secs = (1u64 << consecutive_failures.min(6))
                            .min(MAX_BACKOFF_SECS);
                        warn!(
                            node_id = %node_id,
                            error = %e,
                            consecutive_failures,
                            backoff_secs,
                            "heartbeat failed; backing off"
                        );

                        // Emit warning if disconnected for too long.
                        if last_success.elapsed() > Duration::from_secs(120) {
                            warn!(
                                node_id = %node_id,
                                elapsed_secs = last_success.elapsed().as_secs(),
                                "panel connection lost for >2min"
                            );
                        }

                        // Pause before retrying.
                        tokio::select! {
                            _ = &mut shutdown => break,
                            _ = tokio::time::sleep(Duration::from_secs(backoff_secs)) => {}
                        }
                    }
                }
            }
            _ = cmd_tick.tick(), if pull_commands && consecutive_failures == 0 => {
                match fetch_commands(&client, &commands_url, &api_key).await {
                    Ok(commands) => {
                        for cmd in &commands {
                            execute_panel_command(&command_service, cmd).await;
                        }
                    }
                    Err(e) => debug!(error = %e, "failed to fetch panel commands"),
                }
            }
        }
    }

    info!(node_id = %node_id, "panel connector stopped");
}

async fn send_heartbeat(
    client: &reqwest::Client,
    url: &str,
    api_key: &str,
    hb: &Heartbeat,
) -> Result<serde_json::Value, reqwest::Error> {
    client
        .post(url)
        .header("Authorization", format!("Bearer {api_key}"))
        .json(hb)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await
}

async fn fetch_commands(
    client: &reqwest::Client,
    url: &str,
    api_key: &str,
) -> Result<Vec<serde_json::Value>, reqwest::Error> {
    client
        .get(url)
        .header("Authorization", format!("Bearer {api_key}"))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await
}

async fn execute_panel_command(service: &impl CommandService, cmd: &serde_json::Value) {
    let Some(method) = cmd.get("method").and_then(|m| m.as_str()) else {
        warn!(?cmd, "panel command missing `method` field");
        return;
    };

    let params = cmd
        .get("params")
        .cloned()
        .unwrap_or(serde_json::Value::Null);

    let request = match method {
        "policies.select" => {
            let policy_tag = params["policy_tag"].as_str();
            let target_tag = params["target_tag"].as_str();
            match (policy_tag, target_tag) {
                (Some(p), Some(t)) => CommandRequest::PolicySelect(PolicySelectCommand {
                    policy_tag: p.to_owned(),
                    target_tag: t.to_owned(),
                }),
                _ => {
                    warn!("panel policies.select missing policy_tag or target_tag");
                    return;
                }
            }
        }
        "policies.probe" => {
            let policy_tag = match params["policy_tag"].as_str() {
                Some(p) => p.to_owned(),
                None => {
                    warn!("panel policies.probe missing policy_tag");
                    return;
                }
            };
            CommandRequest::PolicyProbe(PolicyProbeCommand { policy_tag })
        }
        "mode.set" => {
            let mode = match params["mode"].as_str() {
                Some(m) => m.to_owned(),
                None => {
                    warn!("panel mode.set missing mode");
                    return;
                }
            };
            let outbound = params["outbound"].as_str().map(|s| s.to_owned());
            CommandRequest::ModeSet(ModeSetCommand { mode, outbound })
        }
        "flows.close" => {
            let flow_id = match params["flow_id"].as_str() {
                Some(id) => id.to_owned(),
                None => {
                    warn!("panel flows.close missing flow_id");
                    return;
                }
            };
            CommandRequest::FlowClose(FlowCloseCommand { flow_id })
        }
        "tun.start" => {
            let addr = match params["addr"].as_str() {
                Some(a) => a.to_owned(),
                None => {
                    warn!("panel tun.start missing addr");
                    return;
                }
            };
            let tag = match params["tag"].as_str() {
                Some(t) => t.to_owned(),
                None => {
                    warn!("panel tun.start missing tag");
                    return;
                }
            };
            CommandRequest::TunStart(TunStartCommand {
                name: params["name"].as_str().map(|s| s.to_owned()),
                addr,
                mtu: params["mtu"].as_u64().unwrap_or(1500) as u16,
                mask: params["mask"]
                    .as_str()
                    .unwrap_or("255.255.255.0")
                    .to_owned(),
                tag,
            })
        }
        "tun.stop" => CommandRequest::TunStop(TunStopCommand),
        "diagnostics.probe_target" => {
            let target_tag = match params["target_tag"].as_str() {
                Some(t) => t.to_owned(),
                None => {
                    warn!("panel diagnostics.probe_target missing target_tag");
                    return;
                }
            };
            CommandRequest::DiagnosticsProbeTarget(DiagnosticsProbeTargetCommand { target_tag })
        }
        "diagnostics.dns_lookup" => {
            let hostname = match params["hostname"].as_str() {
                Some(h) => h.to_owned(),
                None => {
                    warn!("panel diagnostics.dns_lookup missing hostname");
                    return;
                }
            };
            CommandRequest::DiagnosticsDnsLookup(DiagnosticsDnsLookupCommand { hostname })
        }
        "diagnostics.trace_route" => {
            let target = match params["target"].as_str() {
                Some(t) => t.to_owned(),
                None => {
                    warn!("panel diagnostics.trace_route missing target");
                    return;
                }
            };
            let port = params["port"].as_u64().unwrap_or(443) as u16;
            let protocol = params["protocol"].as_str().map(|s| s.to_owned());
            CommandRequest::DiagnosticsTraceRoute(DiagnosticsTraceRouteCommand {
                target,
                port,
                protocol,
            })
        }
        // Intentionally NOT exposed remotely:
        //   "config.validate" — can be done locally
        //   "config.apply"    — full config replacement from remote is too dangerous
        other => {
            debug!(method = other, "unknown panel command method");
            return;
        }
    };

    match service.execute(request) {
        Ok(_) => debug!(method, "panel command executed"),
        Err(e) => warn!(method, error = %e, "panel command failed"),
    }
}

fn resolve_api_key(
    api_key: &Option<String>,
    api_key_env: &Option<String>,
) -> ConnectorResult<String> {
    if let Some(key) = api_key {
        return Ok(key.clone());
    }
    if let Some(name) = api_key_env {
        let value = std::env::var(name).map_err(|source| ConnectorError::ReadApiKeyEnv {
            name: name.clone(),
            source,
        })?;
        if value.trim().is_empty() {
            return Err(ConnectorError::EmptyApiKeyEnv { name: name.clone() });
        }
        return Ok(value);
    }
    Err(ConnectorError::Config(
        "panel connector requires `api_key` or `api_key_env`".to_owned(),
    ))
}

#[derive(Debug, Serialize)]
struct Heartbeat {
    node_id: String,
    build_id: String,
    uptime_seconds: u64,
    active_flows: usize,
    bytes_up: u64,
    bytes_down: u64,
}
