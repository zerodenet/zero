use std::sync::atomic::Ordering;
use std::time::{SystemTime, UNIX_EPOCH};

use zero_core::Session;

use super::Engine;
use crate::completed_sessions::CompletedSessionRecord;
use crate::hook::{FlowContext, FlowHook, FlowTraffic};
use crate::session_lifecycle::SessionHandle;
use crate::stats::SessionOutcome;
use crate::EngineError;

impl Engine {
    pub fn prepare_session(&self, session: &mut Session, inbound_tag: &str) {
        session.id = self.next_session_id.fetch_add(1, Ordering::Relaxed);
        session.inbound_tag = Some(inbound_tag.to_owned());
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let mode = self.mode.lock().unwrap_or_else(|error| error.into_inner());
        if let Some(hook) = &self.flow_hook {
            let context = FlowContext::from_session(session, mode.kind(), now_ms);
            if let Err(reason) = hook.on_flow_start(&context) {
                tracing::warn!(flow_id = session.id, reason = %reason.message, "flow blocked by hook");
                self.stats.record_finish(SessionOutcome::Cancelled);
                return;
            }
        }
        self.session_registry.insert(session, mode.kind());
        self.stats.record_start();
        self.event_log.push_flow_started(session, mode.kind());
    }

    pub fn set_session_outbound(&self, session: &Session) {
        self.session_registry
            .update_outbound_tag(session.id, session.outbound_tag.as_deref());
    }

    pub fn record_session_upload(&self, id: u64, bytes: u64) {
        self.session_registry.record_upload(id, bytes);
    }
    pub fn record_session_download(&self, id: u64, bytes: u64) {
        self.session_registry.record_download(id, bytes);
    }
    pub fn record_session_inbound_rx(&self, id: u64, bytes: u64) {
        self.session_registry.record_inbound_rx(id, bytes);
    }
    pub fn record_session_inbound_tx(&self, id: u64, bytes: u64) {
        self.session_registry.record_inbound_tx(id, bytes);
    }
    pub fn record_session_outbound_rx(&self, id: u64, bytes: u64) {
        self.session_registry.record_outbound_rx(id, bytes);
    }
    pub fn record_session_outbound_tx(&self, id: u64, bytes: u64) {
        self.session_registry.record_outbound_tx(id, bytes);
    }
    pub fn record_udp_upstream_association_created(&self) {
        self.stats.record_udp_upstream_association_created();
    }
    pub fn record_udp_upstream_association_reused(&self) {
        self.stats.record_udp_upstream_association_reused();
    }
    pub fn record_udp_upstream_association_closed(&self) {
        self.stats.record_udp_upstream_association_closed();
    }
    pub fn record_udp_upstream_association_idle_timeout(&self) {
        self.stats.record_udp_upstream_association_idle_timeout();
    }
    pub fn record_udp_upstream_association_dropped(&self) {
        self.stats.record_udp_upstream_association_dropped();
    }
    pub fn record_udp_upstream_association_failed(&self) {
        self.stats.record_udp_upstream_association_failed();
    }
    pub fn record_udp_upstream_send_failure(&self) {
        self.stats.record_udp_upstream_send_failure();
    }
    pub fn record_udp_upstream_recv_failure(&self) {
        self.stats.record_udp_upstream_recv_failure();
    }
    pub fn record_udp_upstream_packet_sent(&self) {
        self.stats.record_udp_upstream_packet_sent();
    }
    pub fn record_udp_upstream_packet_received(&self) {
        self.stats.record_udp_upstream_packet_received();
    }

    pub fn finish_session(
        &self,
        id: u64,
        outcome: SessionOutcome,
    ) -> Option<CompletedSessionRecord> {
        self.finish_session_with_reason(id, outcome, None)
    }

    pub fn finish_session_with_reason(
        &self,
        id: u64,
        outcome: SessionOutcome,
        reason: Option<String>,
    ) -> Option<CompletedSessionRecord> {
        let record = self.session_registry.finish(id, outcome, reason)?;
        self.stats.record_finish(outcome);
        self.stats.record_traffic(
            record.outbound_tag.as_deref(),
            record.bytes_up,
            record.bytes_down,
        );
        self.completed_sessions.push(record.clone());
        self.event_log
            .push_flow_completed(&record, |tag| self.outbound_protocol_for_tag(tag));
        if let Some(hook) = &self.flow_hook {
            hook.on_flow_end(
                &FlowContext::from_completed(&record),
                outcome,
                &FlowTraffic::from_completed(&record),
            );
        }
        Some(record)
    }

    pub fn track_session(&self, id: u64) -> SessionHandle {
        SessionHandle::new(self.clone(), id)
    }
    pub fn check_outbound_health(&self, tag: &str) -> Result<(), EngineError> {
        self.outbound_health.check(tag)
    }
    pub fn record_outbound_failure(&self, tag: &str) {
        self.outbound_health.record_failure(tag);
    }
    pub fn record_outbound_success(&self, tag: &str) {
        self.outbound_health.record_success(tag);
    }
    pub fn probe_trigger_registry(&self) -> &crate::probe_trigger::ProbeTriggerRegistry {
        &self.probe_trigger_registry
    }

    pub fn trigger_urltest_probe(&self, tag: &str) -> Result<(), EngineError> {
        self.probe_trigger_registry
            .get(tag)
            .ok_or_else(|| EngineError::SelectorGroupNotFound {
                tag: tag.to_owned(),
            })?
            .trigger();
        Ok(())
    }

    pub fn close_flow(&self, flow_id: &str) -> Result<(), EngineError> {
        let id = flow_id.parse().map_err(|_| {
            EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "invalid flow id",
            ))
        })?;
        self.finish_session_with_reason(id, SessionOutcome::Cancelled, Some("manual".to_owned()))
            .map(|_| ())
            .ok_or_else(|| {
                EngineError::Io(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("flow `{flow_id}` not found or already completed"),
                ))
            })
    }

    fn outbound_protocol_for_tag(&self, tag: &str) -> Option<&'static str> {
        if tag == "direct" {
            return Some("direct");
        }
        if tag == "block" {
            return Some("block");
        }
        self.config()
            .outbounds
            .iter()
            .find(|outbound| outbound.tag == tag)
            .map(|outbound| match outbound.protocol {
                zero_config::OutboundProtocolConfig::Direct => "direct",
                zero_config::OutboundProtocolConfig::Block => "block",
                zero_config::OutboundProtocolConfig::Socks5 { .. } => "socks5",
                zero_config::OutboundProtocolConfig::Vless { .. } => "vless",
                zero_config::OutboundProtocolConfig::Hysteria2 { .. } => "hysteria2",
                zero_config::OutboundProtocolConfig::Shadowsocks { .. } => "shadowsocks",
                zero_config::OutboundProtocolConfig::Trojan { .. } => "trojan",
                zero_config::OutboundProtocolConfig::Vmess { .. } => "vmess",
                zero_config::OutboundProtocolConfig::Mieru { .. } => "mieru",
            })
    }
}
