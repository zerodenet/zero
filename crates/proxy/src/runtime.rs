use std::future::Future;
use std::io;
use std::ops::Deref;
use std::path::Path;
use std::sync::Arc;

use tokio::sync::{oneshot, watch};
use tokio::task::{JoinHandle, JoinSet};
use tracing::{info, warn};
use zero_config::{ClientTlsConfig, InboundConfig, InboundProtocolConfig, RuntimeConfig};
use zero_core::Session;
use zero_engine::{
    Engine, EngineError, ResolvedOutbound, RouteDecision, SessionHandle, TargetId,
    UrlTestMemberState,
};
use zero_platform_tokio::{TokioListener, TokioResolver};

use crate::inventory::ProtocolInventory;
#[cfg(feature = "outbound-socks5")]
use crate::transport::MeteredStream;
use crate::transport::{StreamTraffic, TcpRelayStream};

#[derive(Debug, Clone)]
pub struct Proxy {
    engine: Engine,
    pub(crate) config: Arc<RuntimeConfig>,
    pub(crate) resolver: TokioResolver,
    pub(crate) protocols: ProtocolInventory,
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
        protocols.validate_config(engine.config())?;
        Ok(Self {
            config: Arc::new(engine.config().clone()),
            engine,
            resolver: TokioResolver,
            protocols,
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
        let mut urltests: JoinSet<Result<(), EngineError>> = JoinSet::new();

        for inbound in &self.config.inbounds {
            self.spawn_inbound_listener(inbound, &shutdown_rx, &mut listeners)?;
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
                    let _ = shutdown_tx.send(true);
                    info!("propagated proxy shutdown to background tasks");
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

    fn spawn_inbound_listener(
        &self,
        inbound: &InboundConfig,
        shutdown_rx: &watch::Receiver<bool>,
        listeners: &mut JoinSet<Result<(), EngineError>>,
    ) -> Result<(), EngineError> {
        match inbound.protocol {
            InboundProtocolConfig::Socks5 { .. } => {
                #[cfg(feature = "inbound-socks5")]
                {
                    let proxy = self.clone();
                    let inbound = inbound.clone();
                    let shutdown = shutdown_rx.clone();
                    listeners
                        .spawn(async move { proxy.run_socks5_listener(inbound, shutdown).await });
                    Ok(())
                }
                #[cfg(not(feature = "inbound-socks5"))]
                {
                    Err(EngineError::CompiledFeatureDisabled {
                        kind: "inbound",
                        tag: inbound.tag.clone(),
                        protocol: "socks5",
                        feature: "inbound-socks5",
                    })
                }
            }
            InboundProtocolConfig::HttpConnect => {
                #[cfg(feature = "inbound-http-connect")]
                {
                    let proxy = self.clone();
                    let inbound = inbound.clone();
                    let shutdown = shutdown_rx.clone();
                    listeners.spawn(async move {
                        proxy.run_http_connect_listener(inbound, shutdown).await
                    });
                    Ok(())
                }
                #[cfg(not(feature = "inbound-http-connect"))]
                {
                    Err(EngineError::CompiledFeatureDisabled {
                        kind: "inbound",
                        tag: inbound.tag.clone(),
                        protocol: "http-connect",
                        feature: "inbound-http-connect",
                    })
                }
            }
            InboundProtocolConfig::Mixed { .. } => {
                #[cfg(feature = "inbound-mixed")]
                {
                    let proxy = self.clone();
                    let inbound = inbound.clone();
                    let shutdown = shutdown_rx.clone();
                    listeners
                        .spawn(async move { proxy.run_mixed_listener(inbound, shutdown).await });
                    Ok(())
                }
                #[cfg(not(feature = "inbound-mixed"))]
                {
                    Err(EngineError::CompiledFeatureDisabled {
                        kind: "inbound",
                        tag: inbound.tag.clone(),
                        protocol: "mixed",
                        feature: "inbound-mixed",
                    })
                }
            }
            InboundProtocolConfig::Vless { .. } => {
                #[cfg(feature = "inbound-vless")]
                {
                    let proxy = self.clone();
                    let inbound = inbound.clone();
                    let shutdown = shutdown_rx.clone();
                    listeners
                        .spawn(async move { proxy.run_vless_listener(inbound, shutdown).await });
                    Ok(())
                }
                #[cfg(not(feature = "inbound-vless"))]
                {
                    Err(EngineError::CompiledFeatureDisabled {
                        kind: "inbound",
                        tag: inbound.tag.clone(),
                        protocol: "vless",
                        feature: "inbound-vless",
                    })
                }
            }
        }
    }

    pub(crate) fn route_decision<'a>(&'a self, address: &zero_core::Address) -> RouteDecision<'a> {
        self.engine.route_decision(address)
    }

    pub(crate) fn resolve_outbound<'a>(
        &'a self,
        action: RouteDecision<'a>,
    ) -> Result<ResolvedOutbound<'a>, EngineError> {
        self.engine.resolve_route_decision(action)
    }

    pub(crate) fn resolve_target_id<'a>(
        &'a self,
        target_id: TargetId,
    ) -> Option<ResolvedOutbound<'a>> {
        self.engine.resolve_target_id(target_id)
    }

    pub(crate) fn resolve_target_chains(&self, target_id: TargetId) -> Vec<Vec<TargetId>> {
        self.engine.resolve_target_chains(target_id)
    }

    pub(crate) fn target_tag(&self, target_id: TargetId) -> Option<&str> {
        self.engine.target_tag(target_id)
    }

    pub(crate) fn urltest_selected_target(&self, group_id: TargetId) -> Option<TargetId> {
        self.engine.urltest_selected_target(group_id)
    }

    pub(crate) fn update_urltest_state(
        &self,
        group_id: TargetId,
        selected: TargetId,
        latency_ms: Option<u64>,
        members: Vec<UrlTestMemberState>,
    ) {
        self.engine
            .update_urltest_state(group_id, selected, latency_ms, members);
    }

    pub(crate) fn prepare_session(&self, session: &mut Session, inbound_tag: &str) {
        self.engine.prepare_session(session, inbound_tag);
    }

    pub(crate) fn set_session_outbound(&self, session: &Session) {
        self.engine.set_session_outbound(session);
    }

    pub(crate) fn track_session(&self, session_id: u64) -> SessionHandle {
        self.engine.track_session(session_id)
    }

    pub(crate) fn record_session_upload(&self, session_id: u64, bytes: u64) {
        self.engine.record_session_upload(session_id, bytes);
    }

    pub(crate) fn record_session_download(&self, session_id: u64, bytes: u64) {
        self.engine.record_session_download(session_id, bytes);
    }

    pub(crate) fn record_session_inbound_rx(&self, session_id: u64, bytes: u64) {
        self.engine.record_session_inbound_rx(session_id, bytes);
    }

    pub(crate) fn record_session_inbound_tx(&self, session_id: u64, bytes: u64) {
        self.engine.record_session_inbound_tx(session_id, bytes);
    }

    #[cfg(any(feature = "outbound-socks5", feature = "outbound-vless"))]
    pub(crate) fn record_session_outbound_rx(&self, session_id: u64, bytes: u64) {
        self.engine.record_session_outbound_rx(session_id, bytes);
    }

    #[cfg(any(feature = "outbound-socks5", feature = "outbound-vless"))]
    pub(crate) fn record_session_outbound_tx(&self, session_id: u64, bytes: u64) {
        self.engine.record_session_outbound_tx(session_id, bytes);
    }

    pub(crate) fn record_session_inbound_traffic(&self, session_id: u64, traffic: StreamTraffic) {
        if traffic.is_empty() {
            return;
        }

        self.record_session_inbound_rx(session_id, traffic.read_bytes);
        self.record_session_inbound_tx(session_id, traffic.written_bytes);
    }

    #[cfg(any(feature = "outbound-socks5", feature = "outbound-vless"))]
    pub(crate) fn record_session_outbound_traffic(&self, session_id: u64, traffic: StreamTraffic) {
        if traffic.is_empty() {
            return;
        }

        self.record_session_outbound_rx(session_id, traffic.read_bytes);
        self.record_session_outbound_tx(session_id, traffic.written_bytes);
    }

    #[cfg(feature = "inbound-socks5")]
    pub(crate) fn record_udp_upstream_association_created(&self) {
        self.engine.record_udp_upstream_association_created();
    }

    #[cfg(feature = "inbound-socks5")]
    pub(crate) fn record_udp_upstream_association_reused(&self) {
        self.engine.record_udp_upstream_association_reused();
    }

    #[cfg(feature = "outbound-socks5")]
    pub(crate) fn record_udp_upstream_association_closed(&self) {
        self.engine.record_udp_upstream_association_closed();
    }

    #[cfg(feature = "outbound-socks5")]
    pub(crate) fn record_udp_upstream_association_idle_timeout(&self) {
        self.engine.record_udp_upstream_association_idle_timeout();
    }

    #[cfg(feature = "outbound-socks5")]
    pub(crate) fn record_udp_upstream_association_dropped(&self) {
        self.engine.record_udp_upstream_association_dropped();
    }

    #[cfg(feature = "inbound-socks5")]
    pub(crate) fn record_udp_upstream_association_failed(&self) {
        self.engine.record_udp_upstream_association_failed();
    }

    #[cfg(feature = "inbound-socks5")]
    pub(crate) fn record_udp_upstream_send_failure(&self) {
        self.engine.record_udp_upstream_send_failure();
    }

    #[cfg(feature = "inbound-socks5")]
    pub(crate) fn record_udp_upstream_recv_failure(&self) {
        self.engine.record_udp_upstream_recv_failure();
    }

    #[cfg(feature = "inbound-socks5")]
    pub(crate) fn record_udp_upstream_packet_sent(&self) {
        self.engine.record_udp_upstream_packet_sent();
    }

    #[cfg(feature = "inbound-socks5")]
    pub(crate) fn record_udp_upstream_packet_received(&self) {
        self.engine.record_udp_upstream_packet_received();
    }

    #[cfg(feature = "inbound-socks5")]
    pub(crate) fn udp_upstream_idle_timeout(&self) -> std::time::Duration {
        self.engine.udp_upstream_idle_timeout()
    }

    #[cfg(feature = "outbound-socks5")]
    pub(crate) async fn connect_via_socks5_upstream(
        &self,
        session: &zero_core::Session,
        server: &str,
        port: u16,
        auth: Option<(&str, &str)>,
    ) -> Result<TcpRelayStream, EngineError> {
        let upstream = self
            .protocols
            .direct_outbound
            .connect_host(server, port, &self.resolver)
            .await?;
        let mut upstream = MeteredStream::new(upstream);

        self.protocols
            .socks5_outbound
            .establish_tunnel_with_auth(
                &mut upstream,
                session,
                auth.map(
                    |(username, password)| zero_protocol_socks5::Socks5OutboundAuth {
                        username,
                        password,
                    },
                ),
            )
            .await?;
        self.record_session_outbound_traffic(session.id, upstream.drain_traffic());

        Ok(upstream.into_inner().into())
    }

    #[cfg(not(feature = "outbound-socks5"))]
    pub(crate) async fn connect_via_socks5_upstream(
        &self,
        _session: &zero_core::Session,
        _server: &str,
        _port: u16,
        _auth: Option<(&str, &str)>,
    ) -> Result<TcpRelayStream, EngineError> {
        Err(EngineError::CompiledFeatureDisabled {
            kind: "outbound",
            tag: "socks5-upstream".to_owned(),
            protocol: "socks5",
            feature: "outbound-socks5",
        })
    }

    #[cfg(feature = "outbound-vless")]
    pub(crate) async fn connect_via_vless_upstream(
        &self,
        session: &zero_core::Session,
        server: &str,
        port: u16,
        id: &str,
        tls: Option<&ClientTlsConfig>,
        ws: Option<&zero_config::WebSocketConfig>,
    ) -> Result<TcpRelayStream, EngineError> {
        let id = zero_protocol_vless::parse_uuid(id)?;
        let socket = self
            .protocols
            .direct_outbound
            .connect_host(server, port, &self.resolver)
            .await?;

        let stream = match (tls, ws) {
            (Some(tls), Some(ws)) => {
                let tls_stream = crate::transport::connect_tls_upstream(
                    socket,
                    tls,
                    self.config.source_dir(),
                    server,
                )
                .await?;
                match tls_stream {
                    TcpRelayStream::Tls(tls_inner) => {
                        let ws_stream =
                            crate::transport::connect_ws(*tls_inner, ws, server, port).await?;
                        TcpRelayStream::WsTls(Box::new(ws_stream))
                    }
                    _ => unreachable!("connect_tls_upstream always returns Tls variant"),
                }
            }
            (Some(tls), None) => {
                crate::transport::connect_tls_upstream(
                    socket,
                    tls,
                    self.config.source_dir(),
                    server,
                )
                .await?
            }
            (None, Some(ws)) => {
                let ws_stream = crate::transport::connect_ws(socket, ws, server, port).await?;
                TcpRelayStream::WsPlain(Box::new(ws_stream))
            }
            (None, None) => socket.into(),
        };

        let mut upstream = crate::transport::MeteredStream::new(stream);

        self.protocols
            .vless_outbound
            .establish_tcp_tunnel(&mut upstream, session, &id)
            .await?;
        self.record_session_outbound_traffic(session.id, upstream.drain_traffic());

        Ok(upstream.into_inner())
    }

    #[cfg(not(feature = "outbound-vless"))]
    pub(crate) async fn connect_via_vless_upstream(
        &self,
        _session: &zero_core::Session,
        _server: &str,
        _port: u16,
        _id: &str,
        _tls: Option<&ClientTlsConfig>,
        _ws: Option<&zero_config::WebSocketConfig>,
    ) -> Result<TcpRelayStream, EngineError> {
        Err(EngineError::CompiledFeatureDisabled {
            kind: "outbound",
            tag: "vless-upstream".to_owned(),
            protocol: "vless",
            feature: "outbound-vless",
        })
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

pub(crate) async fn bind_listener(inbound: &InboundConfig) -> io::Result<TokioListener> {
    let listen = format!("{}:{}", inbound.listen.address, inbound.listen.port);
    TokioListener::bind(&listen).await
}
