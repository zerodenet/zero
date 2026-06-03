use std::sync::Arc;

use zero_core::{Address, Session};
use zero_engine::{
    EngineError, EnginePlan, ResolvedOutbound, RouteDecision, SessionHandle, TargetId,
    UrlTestMemberState,
};

use crate::runtime::Proxy;
use crate::transport::StreamTraffic;

impl Proxy {
    pub(crate) fn route_decision(&self, session: &zero_core::Session) -> RouteDecision {
        self.engine
            .route_decision(&session.target, session.sni.as_deref())
    }

    pub(crate) fn resolve_outbound(
        &self,
        action: &RouteDecision,
    ) -> Result<(ResolvedOutbound<'static>, Option<Arc<EnginePlan>>), EngineError> {
        self.engine.resolve_route_decision(action.clone())
    }

    pub(crate) fn resolve_target_id(
        &self,
        target_id: TargetId,
    ) -> Option<(ResolvedOutbound<'static>, Arc<EnginePlan>)> {
        self.engine.resolve_target_id(target_id)
    }

    pub(crate) fn resolve_target_chains(&self, target_id: TargetId) -> Vec<Vec<TargetId>> {
        self.engine.resolve_target_chains(target_id)
    }

    pub(crate) fn target_tag(&self, target_id: TargetId) -> Option<String> {
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

    pub(crate) fn prepare_session(
        &self,
        session: &mut Session,
        inbound_tag: &str,
        source_addr: Option<std::net::SocketAddr>,
    ) {
        if let Some(addr) = source_addr {
            session.source_ip = Some(match addr.ip() {
                std::net::IpAddr::V4(v4) => Address::Ipv4(v4.octets()),
                std::net::IpAddr::V6(v6) => Address::Ipv6(v6.octets()),
            });
            session.source_port = Some(addr.port());
        }
        self.engine.prepare_session(session, inbound_tag);

        // Resolve local process identity from the client's source address.
        if let Some(addr) = source_addr {
            if let Some(info) = crate::process_lookup::lookup_process(addr) {
                session.process_id = Some(info.pid);
                session.process_name = Some(info.name);
            }
        }
    }

    /// If the session target is a fake IP, replace it with the real domain
    /// so routing sees the original domain name.
    pub(crate) async fn resolve_fake_ip_target(&self, session: &mut Session) {
        use zero_core::Address;
        use zero_traits::IpAddress;
        let ip = match &session.target {
            Address::Ipv4(octets) => IpAddress::V4(*octets),
            Address::Ipv6(octets) => IpAddress::V6(*octets),
            _ => return,
        };
        if let Some(domain) = self.resolver.lookup_fake_ip(&ip).await {
            session.target = Address::Domain(domain);
        }
    }

    pub(crate) fn set_session_outbound(&self, session: &Session) {
        self.engine.set_session_outbound(session);
    }

    pub(crate) fn track_session(&self, session_id: u64) -> SessionHandle {
        self.engine.track_session(session_id)
    }

    pub(crate) fn record_session_inbound_rx(&self, session_id: u64, bytes: u64) {
        self.engine.record_session_inbound_rx(session_id, bytes);
    }

    pub(crate) fn record_session_inbound_tx(&self, session_id: u64, bytes: u64) {
        self.engine.record_session_inbound_tx(session_id, bytes);
    }

    #[cfg(any(feature = "socks5", feature = "vless"))]
    pub(crate) fn record_session_outbound_rx(&self, session_id: u64, bytes: u64) {
        self.engine.record_session_outbound_rx(session_id, bytes);
    }

    #[cfg(any(feature = "socks5", feature = "vless"))]
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

    #[cfg(any(feature = "socks5", feature = "vless"))]
    pub(crate) fn record_session_outbound_traffic(&self, session_id: u64, traffic: StreamTraffic) {
        if traffic.is_empty() {
            return;
        }

        self.record_session_outbound_rx(session_id, traffic.read_bytes);
        self.record_session_outbound_tx(session_id, traffic.written_bytes);
    }

    #[cfg(feature = "socks5")]
    pub(crate) fn record_udp_upstream_association_created(&self) {
        self.engine.record_udp_upstream_association_created();
    }

    #[cfg(feature = "socks5")]
    pub(crate) fn record_udp_upstream_association_reused(&self) {
        self.engine.record_udp_upstream_association_reused();
    }

    #[cfg(feature = "socks5")]
    pub(crate) fn record_udp_upstream_association_closed(&self) {
        self.engine.record_udp_upstream_association_closed();
    }

    #[cfg(feature = "socks5")]
    pub(crate) fn record_udp_upstream_association_idle_timeout(&self) {
        self.engine.record_udp_upstream_association_idle_timeout();
    }

    #[cfg(feature = "socks5")]
    pub(crate) fn record_udp_upstream_association_dropped(&self) {
        self.engine.record_udp_upstream_association_dropped();
    }

    #[cfg(feature = "socks5")]
    pub(crate) fn record_udp_upstream_association_failed(&self) {
        self.engine.record_udp_upstream_association_failed();
    }

    #[cfg(feature = "socks5")]
    pub(crate) fn record_udp_upstream_send_failure(&self) {
        self.engine.record_udp_upstream_send_failure();
    }

    #[cfg(feature = "socks5")]
    pub(crate) fn record_udp_upstream_recv_failure(&self) {
        self.engine.record_udp_upstream_recv_failure();
    }

    #[cfg(feature = "socks5")]
    pub(crate) fn record_udp_upstream_packet_sent(&self) {
        self.engine.record_udp_upstream_packet_sent();
    }

    #[cfg(feature = "socks5")]
    pub(crate) fn record_udp_upstream_packet_received(&self) {
        self.engine.record_udp_upstream_packet_received();
    }

    #[cfg(feature = "socks5")]
    pub(crate) fn udp_upstream_idle_timeout(&self) -> std::time::Duration {
        self.engine.udp_upstream_idle_timeout()
    }

    pub(crate) fn check_outbound_health(&self, tag: &str) -> Result<(), EngineError> {
        self.engine.check_outbound_health(tag)
    }

    pub(crate) fn record_outbound_failure(&self, tag: &str) {
        self.engine.record_outbound_failure(tag);
    }

    pub(crate) fn record_outbound_success(&self, tag: &str) {
        self.engine.record_outbound_success(tag);
    }
}
