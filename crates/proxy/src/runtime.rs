use std::future::Future;
use std::ops::Deref;
use std::path::Path;
use std::sync::Arc;

use tokio::sync::oneshot;
use tracing::{info, warn};
use zero_config::RuntimeConfig;
use zero_dns::DnsSystem;
use zero_engine::{Engine, EngineError};
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
use zero_platform_tokio::TokioSocket;

use crate::inventory::ProtocolInventory;

#[cfg(any(feature = "shadowsocks", feature = "hysteria2"))]
pub(crate) mod datagram_udp;
mod engine_facade;
mod handle;
#[cfg(feature = "http")]
pub(crate) mod http_redirect;
#[cfg(feature = "vless")]
pub(crate) mod inbound_fallback;
pub(crate) mod inbound_operation;
pub(crate) mod inbound_route;
pub(crate) mod listener_loop;
mod listeners;
#[cfg(any(feature = "vless", feature = "vmess"))]
pub(crate) mod mux_session;
#[cfg(any(feature = "vless", feature = "vmess"))]
pub(crate) mod mux_tcp;
#[cfg(any(feature = "vless", feature = "vmess"))]
pub(crate) mod mux_udp;
pub(crate) mod orchestration;
#[cfg(any(
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru"
))]
pub(crate) mod packet_session_udp;
pub(crate) mod path;
pub(crate) mod pipe;
mod reload;
mod running;
#[cfg(any(
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru"
))]
pub(crate) mod stream_udp;
pub(crate) mod tcp_dispatch;
pub(crate) mod tcp_ingress;
#[cfg(feature = "socks5")]
pub(crate) mod udp_association;
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
pub(crate) mod udp_delivery;
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
pub(crate) mod udp_dispatch;
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
pub(crate) mod udp_flow;
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
pub(crate) mod udp_ingress;
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
pub(crate) mod udp_socket;

pub use handle::ProxyHandle;
pub use running::RunningProxy;

#[derive(Debug, Clone)]
pub struct Proxy {
    engine: Engine,
    pub(crate) config: Arc<RuntimeConfig>,
    pub(crate) resolver: Arc<DnsSystem>,
    pub(crate) protocols: ProtocolInventory,
    pub(crate) tun_shutdown: Arc<std::sync::Mutex<Option<tokio::sync::watch::Sender<bool>>>>,
    pub(crate) tun_info: Arc<std::sync::Mutex<Option<TunInfo>>>,
}

#[derive(Debug, Clone)]
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
            tun_shutdown: Arc::new(std::sync::Mutex::new(None)),
            tun_info: Arc::new(std::sync::Mutex::new(None)),
        })
    }

    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    #[cfg(any(
        feature = "socks5",
        feature = "vless",
        feature = "hysteria2",
        feature = "shadowsocks",
        feature = "trojan",
        feature = "vmess",
        feature = "mieru"
    ))]
    pub(crate) async fn connect_upstream_host(
        &self,
        server: &str,
        port: u16,
    ) -> Result<TokioSocket, zero_transport::RuntimeError> {
        self.protocols
            .direct_connector()
            .connect_host(server, port, self.resolver.as_ref())
            .await
            .map_err(Into::into)
    }

    #[cfg(any(
        feature = "socks5",
        feature = "vless",
        feature = "hysteria2",
        feature = "shadowsocks",
        feature = "trojan",
        feature = "vmess",
        feature = "mieru"
    ))]
    pub(crate) async fn connect_upstream_host_owned(
        &self,
        server: String,
        port: u16,
    ) -> Result<TokioSocket, zero_transport::RuntimeError> {
        self.connect_upstream_host(&server, port).await
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
        orchestration::run_until(self, shutdown).await
    }
}

impl Deref for Proxy {
    type Target = Engine;

    fn deref(&self) -> &Self::Target {
        &self.engine
    }
}
