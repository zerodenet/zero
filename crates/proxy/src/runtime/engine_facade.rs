use zero_core::Session;
use zero_engine::{
    EngineError, ResolvedOutbound, RouteDecision, SessionHandle, TargetId, UrlTestMemberState,
};

use crate::runtime::Proxy;
use crate::transport::StreamTraffic;

impl Proxy {
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
}
