use std::sync::Arc;

use zero_config::RuntimeConfig;
use zero_dns::DnsSystem;
use zero_engine::Engine;

use crate::inventory::ProtocolInventory;
use crate::runtime::Proxy;

#[derive(Clone)]
pub(crate) struct TcpRuntimeServices {
    engine: Engine,
    config: Arc<RuntimeConfig>,
    resolver: Arc<DnsSystem>,
    protocols: ProtocolInventory,
}

impl TcpRuntimeServices {
    pub(crate) fn new(
        engine: Engine,
        config: Arc<RuntimeConfig>,
        resolver: Arc<DnsSystem>,
        protocols: ProtocolInventory,
    ) -> Self {
        Self {
            engine,
            config,
            resolver,
            protocols,
        }
    }

    pub(crate) fn from_proxy(proxy: &Proxy) -> Self {
        Self::new(
            proxy.engine().clone(),
            proxy.config.clone(),
            proxy.resolver.clone(),
            proxy.protocols.clone(),
        )
    }

    pub(crate) fn engine(&self) -> &Engine {
        &self.engine
    }

    pub(crate) fn config(&self) -> &RuntimeConfig {
        self.config.as_ref()
    }

    pub(crate) fn resolver(&self) -> &DnsSystem {
        self.resolver.as_ref()
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
    pub(crate) async fn connect_upstream_owned(
        &self,
        server: String,
        port: u16,
    ) -> Result<zero_platform_tokio::TokioSocket, zero_transport::RuntimeError> {
        self.protocols
            .direct_connector()
            .connect_host(&server, port, self.resolver.as_ref())
            .await
            .map_err(Into::into)
    }

    pub(crate) async fn connect_direct(
        &self,
        session: &zero_core::Session,
    ) -> Result<zero_platform_tokio::TokioSocket, zero_engine::EngineError> {
        self.protocols
            .direct_connector()
            .connect(session, self.resolver.as_ref())
            .await
            .map_err(Into::into)
    }

    pub(crate) fn check_outbound_health(&self, tag: &str) -> Result<(), zero_engine::EngineError> {
        self.engine.check_outbound_health(tag)
    }

    pub(crate) fn record_outbound_failure(&self, tag: &str) {
        self.engine.record_outbound_failure(tag);
    }

    pub(crate) fn record_outbound_success(&self, tag: &str) {
        self.engine.record_outbound_success(tag);
    }

    pub(crate) fn prepare_tcp_outbound<'a>(
        &self,
        resolved: &'a zero_engine::ResolvedOutbound<'a>,
    ) -> Result<crate::inventory::PreparedTcpOutbound<'a>, crate::transport::TcpOutboundFailure>
    {
        self.protocols.prepare_tcp_outbound(
            OutboundAdapterContext::new(self.config.source_dir()),
            resolved,
        )
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
    pub(crate) async fn dispatch_prepared_tcp_relay_carrier(
        &self,
        prepared: crate::inventory::PreparedTcpRelayChain<'_>,
    ) -> Result<crate::transport::RelayCarrier, crate::transport::TcpOutboundFailure> {
        crate::inventory::dispatch_prepared_tcp_relay_carrier(self.clone(), prepared).await
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
    pub(crate) fn record_control_traffic(
        &self,
        session_id: u64,
        traffic: zero_transport::StreamTraffic,
    ) {
        if traffic.is_empty() {
            return;
        }

        self.record_session_outbound_rx(session_id, traffic.read_bytes);
        self.record_session_outbound_tx(session_id, traffic.written_bytes);
    }

    pub(crate) fn record_session_inbound_rx(&self, session_id: u64, bytes: u64) {
        self.engine.record_session_inbound_rx(session_id, bytes);
    }

    pub(crate) fn record_session_inbound_tx(&self, session_id: u64, bytes: u64) {
        self.engine.record_session_inbound_tx(session_id, bytes);
    }

    pub(crate) fn record_session_outbound_rx(&self, session_id: u64, bytes: u64) {
        self.engine.record_session_outbound_rx(session_id, bytes);
    }

    pub(crate) fn record_session_outbound_tx(&self, session_id: u64, bytes: u64) {
        self.engine.record_session_outbound_tx(session_id, bytes);
    }
}

#[derive(Clone)]
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
pub(crate) struct UdpRuntimeServices {
    proxy: Proxy,
}

#[derive(Clone, Copy)]
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
pub(crate) enum UdpAssociationCloseKind {
    Closed,
    IdleTimeout,
    Dropped,
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
impl UdpRuntimeServices {
    pub(crate) fn from_proxy(proxy: &Proxy) -> Self {
        Self {
            proxy: proxy.clone(),
        }
    }

    pub(crate) async fn connect_upstream(
        &self,
        server: &str,
        port: u16,
    ) -> Result<zero_platform_tokio::TokioSocket, zero_transport::RuntimeError> {
        self.proxy
            .connect_upstream_host_owned(server.to_owned(), port)
            .await
    }

    pub(crate) async fn resolve_udp_peer(
        &self,
        address: &zero_core::Address,
        port: u16,
        context: &'static str,
    ) -> Result<
        (
            zero_traits::SocketAddress,
            zero_platform_tokio::TokioDatagramSocket,
        ),
        zero_transport::RuntimeError,
    > {
        let peer = self
            .proxy
            .protocols
            .direct_connector()
            .resolve_address(address, port, self.proxy.resolver.as_ref(), context)
            .await
            .map_err(zero_transport::RuntimeError::from)?;
        let socket = crate::runtime::udp_socket::bind_datagram_socket_for_peer(peer)
            .await
            .map_err(zero_transport::RuntimeError::from)?;
        Ok((
            zero_platform_tokio::socket_addr_to_socket_address(peer),
            socket,
        ))
    }

    pub(crate) async fn resolve_direct_address(
        &self,
        address: &zero_core::Address,
        port: u16,
        error_message: &'static str,
    ) -> Result<std::net::SocketAddr, zero_engine::EngineError> {
        self.proxy
            .protocols
            .direct_connector()
            .resolve_address(address, port, self.proxy.resolver.as_ref(), error_message)
            .await
            .map_err(Into::into)
    }

    pub(crate) async fn resolve_direct_target(
        &self,
        session: &zero_core::Session,
    ) -> Result<std::net::SocketAddr, zero_engine::EngineError> {
        self.proxy
            .protocols
            .direct_connector()
            .resolve_target_addr(session, self.proxy.resolver.as_ref())
            .await
            .map_err(Into::into)
    }

    pub(crate) async fn dispatch_prepared_tcp_relay_carrier(
        &self,
        prepared: crate::inventory::PreparedTcpRelayChain<'_>,
    ) -> Result<crate::transport::RelayCarrier, crate::transport::TcpOutboundFailure> {
        TcpRuntimeServices::from_proxy(&self.proxy)
            .dispatch_prepared_tcp_relay_carrier(prepared)
            .await
    }

    pub(crate) fn record_control_traffic(
        &self,
        session_id: u64,
        traffic: zero_transport::StreamTraffic,
    ) {
        self.proxy
            .record_session_outbound_traffic(session_id, traffic);
    }

    pub(crate) fn record_session_inbound_rx(&self, session_id: u64, bytes: u64) {
        self.proxy.record_session_inbound_rx(session_id, bytes);
    }

    pub(crate) fn record_session_inbound_tx(&self, session_id: u64, bytes: u64) {
        self.proxy.record_session_inbound_tx(session_id, bytes);
    }

    pub(crate) fn record_session_outbound_rx(&self, session_id: u64, bytes: u64) {
        self.proxy.record_session_outbound_rx(session_id, bytes);
    }

    pub(crate) fn record_session_outbound_tx(&self, session_id: u64, bytes: u64) {
        self.proxy.record_session_outbound_tx(session_id, bytes);
    }

    #[cfg(any(feature = "socks5", feature = "vless"))]
    pub(crate) fn record_session_inbound_traffic(
        &self,
        session_id: u64,
        traffic: zero_transport::StreamTraffic,
    ) {
        self.proxy
            .record_session_inbound_traffic(session_id, traffic);
    }

    pub(crate) fn udp_enabled_for_outbound(&self, outbound_tag: Option<&str>) -> bool {
        self.proxy.udp_enabled_for_outbound(outbound_tag)
    }

    pub(crate) fn udp_upstream_idle_timeout(&self) -> std::time::Duration {
        self.proxy.udp_upstream_idle_timeout()
    }

    pub(crate) fn record_udp_upstream_association_created(&self) {
        self.proxy.record_udp_upstream_association_created();
    }

    pub(crate) fn record_udp_upstream_association_reused(&self) {
        self.proxy.record_udp_upstream_association_reused();
    }

    pub(crate) fn record_udp_upstream_association_failed(&self) {
        self.proxy.record_udp_upstream_association_failed();
    }

    pub(crate) fn record_udp_upstream_send_failure(&self) {
        self.proxy.record_udp_upstream_send_failure();
    }

    pub(crate) fn record_udp_upstream_packet_sent(&self) {
        self.proxy.record_udp_upstream_packet_sent();
    }

    #[cfg(feature = "socks5")]
    pub(crate) fn record_udp_upstream_recv_failure(&self) {
        self.proxy.record_udp_upstream_recv_failure();
    }

    #[cfg(feature = "socks5")]
    pub(crate) fn record_udp_upstream_packet_received(&self) {
        self.proxy.record_udp_upstream_packet_received();
    }

    pub(crate) fn record_association_close(&self, kind: UdpAssociationCloseKind) {
        match kind {
            UdpAssociationCloseKind::Closed => self.proxy.record_udp_upstream_association_closed(),
            UdpAssociationCloseKind::IdleTimeout => {
                self.proxy.record_udp_upstream_association_idle_timeout()
            }
            UdpAssociationCloseKind::Dropped => {
                self.proxy.record_udp_upstream_association_dropped()
            }
        }
    }

    #[cfg(feature = "shadowsocks")]
    pub(crate) async fn build_udp_socket_carrier(
        &self,
        server: &str,
        port: u16,
        codec: std::sync::Arc<
            dyn zero_traits::DatagramCodec<zero_core::Address, Error = zero_core::Error>,
        >,
    ) -> Result<
        std::sync::Arc<dyn crate::runtime::udp_flow::packet_path::PacketPathCarrier>,
        zero_engine::EngineError,
    > {
        crate::runtime::udp_flow::packet_path_chain::carriers::udp_socket_carrier::build(
            self, server, port, codec,
        )
        .await
    }
}

#[derive(Clone)]
pub(crate) struct OutboundAdapterContext {
    source_dir: Option<std::path::PathBuf>,
}

impl OutboundAdapterContext {
    pub(crate) fn new(source_dir: Option<&std::path::Path>) -> Self {
        Self {
            source_dir: source_dir.map(std::path::Path::to_path_buf),
        }
    }

    pub(crate) fn source_dir(&self) -> Option<&std::path::Path> {
        self.source_dir.as_deref()
    }
}

#[derive(Clone)]
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
pub(crate) struct UdpAdapterContext<'a> {
    source_dir: Option<&'a std::path::Path>,
    services: UdpRuntimeServices,
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
impl<'a> UdpAdapterContext<'a> {
    pub(crate) fn new(
        source_dir: Option<&'a std::path::Path>,
        services: UdpRuntimeServices,
    ) -> Self {
        Self {
            source_dir,
            services,
        }
    }

    pub(crate) fn source_dir(&self) -> Option<&'a std::path::Path> {
        self.source_dir
    }

    pub(crate) fn udp_enabled_for_outbound(&self, outbound_tag: Option<&str>) -> bool {
        self.services.udp_enabled_for_outbound(outbound_tag)
    }

    pub(crate) fn runtime_services(&self) -> UdpRuntimeServices {
        self.services.clone()
    }
}
