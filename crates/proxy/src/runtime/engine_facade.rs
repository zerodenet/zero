use std::sync::Arc;

use zero_engine::{EnginePlan, ResolvedOutbound, TargetId, UrlTestMemberState};

use crate::runtime::Proxy;
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
use crate::transport::StreamTraffic;

impl Proxy {
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

    #[cfg(any(feature = "socks5", feature = "vless"))]
    pub(crate) fn record_session_inbound_traffic(&self, session_id: u64, traffic: StreamTraffic) {
        if traffic.is_empty() {
            return;
        }

        self.record_session_inbound_rx(session_id, traffic.read_bytes);
        self.record_session_inbound_tx(session_id, traffic.written_bytes);
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

    #[cfg(any(
        feature = "socks5",
        feature = "vless",
        feature = "vmess",
        feature = "trojan",
        feature = "mieru"
    ))]
    pub(crate) fn udp_upstream_idle_timeout(&self) -> std::time::Duration {
        self.engine.udp_upstream_idle_timeout()
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
    pub(crate) fn udp_enabled_for_outbound(&self, outbound_tag: Option<&str>) -> bool {
        let config = self.engine.config();
        config.runtime.udp.enabled
            && outbound_tag
                .and_then(|tag| config.outbounds.iter().find(|outbound| outbound.tag == tag))
                .map(|outbound| outbound.udp.enabled)
                .unwrap_or(true)
    }
}
