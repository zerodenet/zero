use std::collections::HashMap;
use std::future::Future;
use std::io;
use std::ops::Deref;
use std::path::Path;
use std::sync::Arc;

use tokio::sync::{oneshot, watch};
use tokio::task::{JoinHandle, JoinSet};
use tracing::{info, warn};
use zero_config::RuntimeConfig;
use zero_dns::DnsSystem;
use zero_engine::{Engine, EngineError};

use crate::inventory::ProtocolInventory;
use crate::protocol_runtime::vless_mux_pool::MuxConnectionPool;
#[cfg(feature = "vmess")]
use crate::protocol_runtime::vmess_mux_pool::VmessMuxConnectionPool;

mod engine_facade;
pub(crate) mod inbound_protocol;
mod listeners;
pub(crate) mod orchestration;
pub(crate) mod pipe;
mod tcp_dispatch;
pub(crate) mod udp_dispatch;
pub(crate) mod udp_flow;
pub(crate) mod udp_helpers;

#[derive(Debug, Clone)]
pub struct Proxy {
    engine: Engine,
    pub(crate) config: Arc<RuntimeConfig>,
    pub(crate) resolver: Arc<DnsSystem>,
    pub(crate) protocols: ProtocolInventory,
    pub(crate) mux_pool: MuxConnectionPool,
    #[cfg(feature = "vmess")]
    pub(crate) vmess_mux_pool: VmessMuxConnectionPool,
    pub(crate) tun_shutdown: Arc<std::sync::Mutex<Option<tokio::sync::watch::Sender<bool>>>>,
    pub(crate) tun_info: Arc<std::sync::Mutex<Option<TunInfo>>>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct TunInfo {
    pub name: String,
    pub addr: String,
    pub tag: String,
}

impl Proxy {
    pub fn new(config: RuntimeConfig) -> Result<Self, EngineError> {
        Self::from_engine(Engine::new(config)?)
    }

    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, EngineError> {
        let config = RuntimeConfig::load_from_path(path)?;
        Self::new(config)
    }

    pub fn from_engine(engine: Engine) -> Result<Self, EngineError> {
        let protocols = ProtocolInventory::default();
        let config = engine.config();
        protocols.validate_config(&config)?;
        let dns = DnsSystem::build(config.runtime.dns.as_ref()).map_err(EngineError::Io)?;
        Ok(Self {
            config,
            engine,
            resolver: Arc::new(dns),
            protocols,
            mux_pool: MuxConnectionPool::new(),
            #[cfg(feature = "vmess")]
            vmess_mux_pool: VmessMuxConnectionPool::new(),
            tun_shutdown: Arc::new(std::sync::Mutex::new(None)),
            tun_info: Arc::new(std::sync::Mutex::new(None)),
        })
    }

    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    pub fn with_udp_upstream_idle_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.engine = self.engine.with_udp_upstream_idle_timeout(timeout);
        self
    }

    pub fn into_engine(self) -> Engine {
        self.engine
    }

    pub fn spawn(&self) -> RunningProxy {
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let proxy = self.clone();
        let task = tokio::spawn(async move {
            proxy
                .run_until(async {
                    let _ = shutdown_rx.await;
                })
                .await
        });

        RunningProxy {
            proxy: self.clone(),
            shutdown: Some(shutdown_tx),
            task,
        }
    }

    pub async fn run(&self) -> Result<(), EngineError> {
        self.run_until(async {
            match tokio::signal::ctrl_c().await {
                Ok(()) => info!("shutdown signal received"),
                Err(error) => warn!(error = %error, "failed to listen for ctrl-c; stopping proxy"),
            }
        })
        .await
    }

    pub async fn run_until<F>(&self, shutdown: F) -> Result<(), EngineError>
    where
        F: Future<Output = ()> + Send,
    {
        if self.config.inbounds.is_empty() {
            return Err(EngineError::NoInbounds);
        }

        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let mut listeners: JoinSet<Result<(), EngineError>> = JoinSet::new();
        // Per-listener shutdown channels, keyed by inbound tag.
        let mut listener_stops: HashMap<String, watch::Sender<bool>> = HashMap::new();
        let mut urltests: JoinSet<Result<(), EngineError>> = JoinSet::new();

        let reload_rx = self.engine.subscribe_reload();
        // Bridge std mpsc (blocking) ->?async via a spawn_blocking task.
        // Uses recv_timeout so the thread can detect when the async
        // receiver is dropped (e.g. during shutdown) and exit cleanly
        // instead of blocking tokio runtime teardown.
        let (reload_tx, mut reload_async_rx) = tokio::sync::mpsc::unbounded_channel();
        tokio::task::spawn_blocking(move || loop {
            match reload_rx.recv_timeout(std::time::Duration::from_millis(500)) {
                Ok(()) => {
                    if reload_tx.send(()).is_err() {
                        break; // async receiver dropped
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    if reload_tx.is_closed() {
                        break; // async receiver dropped during shutdown
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    break; // engine dropped all senders
                }
            }
        });

        // Initial listener / urltest population.
        for inbound in &self.config.inbounds {
            let (tx, rx) = watch::channel(false);
            listener_stops.insert(inbound.tag.clone(), tx);
            let bound = listeners::bind_inbound_listener(self, inbound).await?;
            listeners::spawn_inbound_listener(self, inbound, bound, rx, &mut listeners);
        }
        for &group_id in self.engine.plan().urltest_groups() {
            let proxy = self.clone();
            let shutdown = shutdown_rx.clone();
            urltests.spawn(async move { proxy.run_urltest_group(group_id, shutdown).await });
        }

        info!(
            inbound_count = self.config.inbounds.len(),
            outbound_count = self.config.outbounds.len(),
            outbound_group_count = self.config.outbound_groups.len(),
            rule_count = self.config.route.rules.len(),
            mode = %self.config.mode.kind(),
            udp_upstream_idle_timeout_seconds = self.engine.udp_upstream_idle_timeout().as_secs(),
            supported_inbounds = ?self.protocols.supported_inbounds(),
            supported_outbounds = ?self.protocols.supported_outbounds(),
            "zero-proxy started"
        );

        tokio::pin!(shutdown);
        let mut shutting_down = false;

        loop {
            if shutting_down && listeners.is_empty() && urltests.is_empty() {
                let stats = self.engine.stats_snapshot();
                info!(
                    total_started = stats.total_started,
                    completed_sessions = stats.completed_sessions,
                    failed_sessions = stats.failed_sessions,
                    blocked_sessions = stats.blocked_sessions,
                    direct_sessions = stats.direct_sessions,
                    chained_sessions = stats.chained_sessions,
                    udp_upstream_active_associations = stats.udp_upstream.active_associations,
                    udp_upstream_created_associations = stats.udp_upstream.created_associations,
                    udp_upstream_reused_associations = stats.udp_upstream.reused_associations,
                    udp_upstream_closed_associations = stats.udp_upstream.closed_associations,
                    udp_upstream_idle_timeouts = stats.udp_upstream.idle_timeouts,
                    udp_upstream_dropped_associations = stats.udp_upstream.dropped_associations,
                    "zero-proxy stopped"
                );
                return Ok(());
            }

            tokio::select! {
                _ = &mut shutdown, if !shutting_down => {
                    shutting_down = true;
                    // Notify the original channel (used by urltest groups).
                    let _ = shutdown_tx.send(true);
                    // Notify each per-listener channel (used by inbound listeners).
                    for tx in listener_stops.values() {
                        let _ = tx.send(true);
                    }
                    info!("propagated proxy shutdown to background tasks");
                }
                Some(()) = reload_async_rx.recv() => {
                    if shutting_down {
                        continue;
                    }
                    let new_config = self.engine.config();
                    // Reload DNS if config changed.
                    if let Err(e) = self.resolver.reload(new_config.runtime.dns.as_ref()) {
                        warn!(error = %e, "failed to reload dns config");
                    }
                    listeners::reconcile_inbounds(
                        self,
                        &new_config,
                        &mut listener_stops,
                        &mut listeners,
                    ).await;
                    // Remove old urltest groups --they detect config
                    // changes via the plan swap and exit cleanly next cycle.
                    // Spawn new ones.
                    listeners::reconcile_urltests(self, &new_config, &shutdown_rx, &mut urltests);
                    self.mux_pool.evict_all();
                    #[cfg(feature = "vmess")]
                    self.vmess_mux_pool.evict_all();
                    info!(
                        inbound_count = new_config.inbounds.len(),
                        outbound_count = new_config.outbounds.len(),
                        outbound_group_count = new_config.outbound_groups.len(),
                        "config reload reconciled"
                    );
                }
                result = listeners.join_next(), if !listeners.is_empty() => {
                    match result {
                        Some(Ok(Ok(()))) if shutting_down => {}
                        Some(Ok(Ok(()))) => return Err(EngineError::InboundTaskExited),
                        Some(Ok(Err(error))) => return Err(error),
                        Some(Err(error)) => return Err(io::Error::other(error).into()),
                        None if shutting_down => return Ok(()),
                        None => return Err(EngineError::InboundTaskExited),
                    }
                }
                result = urltests.join_next(), if !urltests.is_empty() => {
                    match result {
                        Some(Ok(Ok(()))) if shutting_down => {}
                        Some(Ok(Ok(()))) => return Err(EngineError::UrlTestTaskExited),
                        Some(Ok(Err(error))) => return Err(error),
                        Some(Err(error)) => return Err(io::Error::other(error).into()),
                        None if shutting_down => {}
                        None => return Err(EngineError::UrlTestTaskExited),
                    }
                }
            }
        }
    }
}

impl Deref for Proxy {
    type Target = Engine;

    fn deref(&self) -> &Self::Target {
        &self.engine
    }
}

pub struct RunningProxy {
    proxy: Proxy,
    shutdown: Option<oneshot::Sender<()>>,
    task: JoinHandle<Result<(), EngineError>>,
}

impl RunningProxy {
    pub fn engine(&self) -> &Engine {
        self.proxy.engine()
    }

    pub async fn shutdown(mut self) -> Result<(), EngineError> {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }

        self.task
            .await
            .map_err(|error| EngineError::from(io::Error::other(error)))?
    }
}

impl Deref for RunningProxy {
    type Target = Engine;

    fn deref(&self) -> &Self::Target {
        self.proxy.engine()
    }
}

// //// ProxyHandle: IPC command interception //////////////////////////////////////////////////

/// Wraps [`EngineHandle`] with TUN command interception.
///
/// TUN start/stop commands are handled by the proxy runtime,
/// not the engine.  This wrapper intercepts those commands
/// before they reach `EngineHandle`.
#[derive(Clone)]
pub struct ProxyHandle {
    inner: zero_engine::EngineHandle,
    proxy: Proxy,
}

impl ProxyHandle {
    pub fn new(inner: zero_engine::EngineHandle, proxy: Proxy) -> Self {
        Self { inner, proxy }
    }

    /// Access the underlying EngineHandle.
    pub fn engine_handle(&self) -> &zero_engine::EngineHandle {
        &self.inner
    }
}

impl zero_api::QueryService for ProxyHandle {
    fn query(
        &self,
        request: zero_api::QueryRequest,
    ) -> zero_api::ApiResult<zero_api::QueryResponse> {
        if let zero_api::QueryRequest::Capabilities(_) = &request {
            let response = self.inner.query(request)?;
            let zero_api::QueryResponse::Capabilities(mut capabilities) = response else {
                return Ok(response);
            };
            capabilities.protocols = self.proxy.protocols.protocol_capabilities();
            return Ok(zero_api::QueryResponse::Capabilities(capabilities));
        }
        if let zero_api::QueryRequest::TunStatus(_) = &request {
            let info = self.proxy.tun_info.lock().unwrap();
            let snap = match info.as_ref() {
                Some(tun) => zero_api::TunStatusSnapshot {
                    running: true,
                    name: Some(tun.name.clone()),
                    addr: Some(tun.addr.clone()),
                    tag: Some(tun.tag.clone()),
                },
                None => zero_api::TunStatusSnapshot::default(),
            };
            return Ok(zero_api::QueryResponse::TunStatus(snap));
        }
        self.inner.query(request)
    }
}

impl zero_api::CommandService for ProxyHandle {
    fn execute(
        &self,
        command: zero_api::CommandRequest,
    ) -> zero_api::ApiResult<zero_api::CommandResponse> {
        match &command {
            zero_api::CommandRequest::TunStart(cmd) => {
                let proxy = self.proxy.clone();
                let name = cmd.name.clone();
                let addr = cmd.addr.clone();
                let mask = cmd.mask.clone();
                let mtu = cmd.mtu;
                let tag = cmd.tag.clone();
                match tokio::runtime::Handle::try_current() {
                    Ok(rt) => rt.block_on(async move {
                        proxy
                            .start_tun(name.as_deref(), &addr, &mask, mtu, &tag)
                            .await
                            .map(|_| zero_api::CommandResponse::accepted())
                            .map_err(|e| {
                                zero_api::ApiError::new(
                                    zero_api::ApiErrorCode::Internal,
                                    e.to_string(),
                                )
                            })
                    }),
                    Err(_) => Err(zero_api::ApiError::new(
                        zero_api::ApiErrorCode::Internal,
                        "no tokio runtime available for TUN command",
                    )),
                }
            }
            zero_api::CommandRequest::TunStop(_) => match tokio::runtime::Handle::try_current() {
                Ok(rt) => rt.block_on(async move {
                    self.proxy
                        .stop_tun()
                        .map(|_| zero_api::CommandResponse::accepted())
                        .map_err(|e| {
                            zero_api::ApiError::new(zero_api::ApiErrorCode::Internal, e.to_string())
                        })
                }),
                Err(_) => Err(zero_api::ApiError::new(
                    zero_api::ApiErrorCode::Internal,
                    "no tokio runtime available for TUN command",
                )),
            },
            zero_api::CommandRequest::DiagnosticsProbeOutbound(cmd) => {
                let proxy = self.proxy.clone();
                let target_tag = cmd.target_tag.clone();
                let url = cmd
                    .url
                    .clone()
                    .unwrap_or_else(|| crate::groups::DEFAULT_PROBE_URL.to_owned());
                match tokio::runtime::Handle::try_current() {
                    Ok(rt) => rt.block_on(async move {
                        match proxy.probe_outbound_single(&target_tag, &url).await {
                            Ok(latency_ms) => Ok(zero_api::CommandResponse {
                                accepted: true,
                                result: Some(serde_json::json!({
                                    "target_tag": target_tag,
                                    "url": url,
                                    "via": "through_proxy",
                                    "reachable": true,
                                    "latency_ms": latency_ms,
                                })),
                            }),
                            Err(error) => {
                                // A failed probe (timeout, refused) is a
                                // *result*, not a command error -the node is
                                // simply unreachable, which the GUI renders.
                                Ok(zero_api::CommandResponse {
                                    accepted: true,
                                    result: Some(serde_json::json!({
                                        "target_tag": target_tag,
                                        "url": url,
                                        "via": "through_proxy",
                                        "reachable": false,
                                        "latency_ms": null,
                                        "error": error.to_string(),
                                    })),
                                })
                            }
                        }
                    }),
                    Err(_) => Err(zero_api::ApiError::new(
                        zero_api::ApiErrorCode::Internal,
                        "no tokio runtime available for probe_outbound command",
                    )),
                }
            }
            zero_api::CommandRequest::DiagnosticsDnsCache(cmd) => {
                let proxy = self.proxy.clone();
                let domain = cmd.domain.clone();
                let limit = cmd.limit.unwrap_or(256);
                match tokio::runtime::Handle::try_current() {
                    Ok(rt) => rt.block_on(async move {
                        let resolver = &proxy.resolver;
                        let enabled = resolver.cache_enabled();
                        let result = if let Some(domain) = domain {
                            match resolver.inspect_cache(&domain).await {
                                Some((addresses, ttl_seconds)) => serde_json::json!({
                                    "enabled": enabled,
                                    "domain": domain,
                                    "hit": true,
                                    "addresses": addresses,
                                    "ttl_seconds": ttl_seconds,
                                }),
                                None => serde_json::json!({
                                    "enabled": enabled,
                                    "domain": domain,
                                    "hit": false,
                                    "addresses": [],
                                    "ttl_seconds": null,
                                }),
                            }
                        } else {
                            let entries: Vec<_> = resolver
                                .list_cache(limit)
                                .await
                                .into_iter()
                                .map(|(domain, addresses, ttl_seconds)| {
                                    serde_json::json!({
                                        "domain": domain,
                                        "addresses": addresses,
                                        "ttl_seconds": ttl_seconds,
                                    })
                                })
                                .collect();
                            let count = entries.len();
                            serde_json::json!({
                                "enabled": enabled,
                                "entries": entries,
                                "count": count,
                            })
                        };
                        Ok(zero_api::CommandResponse {
                            accepted: true,
                            result: Some(result),
                        })
                    }),
                    Err(_) => Err(zero_api::ApiError::new(
                        zero_api::ApiErrorCode::Internal,
                        "no tokio runtime available for dns_cache command",
                    )),
                }
            }
            zero_api::CommandRequest::DiagnosticsFakeipLookup(cmd) => {
                let proxy = self.proxy.clone();
                let domain = cmd.domain.clone();
                let ip = cmd.ip.clone();
                match tokio::runtime::Handle::try_current() {
                    Ok(rt) => rt.block_on(async move {
                        let resolver = &proxy.resolver;
                        let enabled = resolver.fake_ip_enabled();
                        let result = if let Some(domain) = domain {
                            let fake_ip = resolver.lookup_fake_ip_domain(&domain).await;
                            serde_json::json!({
                                "enabled": enabled,
                                "domain": domain,
                                "fake_ip": fake_ip,
                            })
                        } else if let Some(ip) = ip {
                            let domain = match parse_ip_address(&ip) {
                                Some(addr) => resolver.lookup_fake_ip(&addr).await,
                                None => {
                                    return Err(zero_api::ApiError::new(
                                        zero_api::ApiErrorCode::InvalidArgument,
                                        format!("invalid ip `{ip}`"),
                                    ))
                                }
                            };
                            serde_json::json!({
                                "enabled": enabled,
                                "ip": ip,
                                "domain": domain,
                            })
                        } else {
                            return Err(zero_api::ApiError::new(
                                zero_api::ApiErrorCode::InvalidArgument,
                                "fakeip_lookup requires `domain` or `ip`",
                            ));
                        };
                        Ok(zero_api::CommandResponse {
                            accepted: true,
                            result: Some(result),
                        })
                    }),
                    Err(_) => Err(zero_api::ApiError::new(
                        zero_api::ApiErrorCode::Internal,
                        "no tokio runtime available for fakeip_lookup command",
                    )),
                }
            }
            _ => self.inner.execute(command),
        }
    }
}

impl zero_api::EventSource for ProxyHandle {
    type Stream = <zero_engine::EngineHandle as zero_api::EventSource>::Stream;

    fn subscribe(&self, filter: zero_api::EventFilter) -> zero_api::ApiResult<Self::Stream> {
        self.inner.subscribe(filter)
    }

    fn latest(
        &self,
        limit: usize,
        filter: zero_api::EventFilter,
    ) -> zero_api::ApiResult<Vec<zero_api::RawApiEvent>> {
        self.inner.latest(limit, filter)
    }
}

// //// listener lifecycle helpers //////////////////////////////////////////////////////////////////////////////

/// Eagerly bind a listener socket via the protocol's registered adapter.
///
/// Port conflicts surface here (before spawn) via `?`, not deferred until
/// `JoinSet::join_next()`. The adapter reads its own protocol config fields
/// (cert/key for QUIC) -the runtime never touches those.
/// Parse a dotted/colon IP string into a `zero_traits::IpAddress` for the
/// fake-IP reverse diagnostic lookup.
fn parse_ip_address(s: &str) -> Option<zero_traits::IpAddress> {
    match s.parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V4(v4)) => Some(zero_traits::IpAddress::V4(v4.octets())),
        Ok(std::net::IpAddr::V6(v6)) => Some(zero_traits::IpAddress::V6(v6.octets())),
        Err(_) => None,
    }
}
