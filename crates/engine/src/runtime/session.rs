use std::sync::atomic::Ordering;
use std::time::{SystemTime, UNIX_EPOCH};

use zero_core::Session;

use super::Engine;
use crate::completed_sessions::CompletedSessionRecord;
use crate::hook::{FlowContext, FlowHook, FlowTraffic};
use crate::session_lifecycle::SessionHandle;
use crate::stats::SessionOutcome;
use crate::{
    EngineError, FlowFailureObservation, FlowRemoteEndpoint, FlowRouteObservation, RouteDecision,
    RouteTrace,
};

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
        if let Some(active) = self.session_registry.snapshot_one(session.id) {
            self.event_log.push_flow_started(&active);
        }
    }

    pub fn set_session_outbound(&self, session: &Session) {
        self.set_session_outbound_with_path(session, None, Vec::new());
    }

    pub fn set_session_outbound_with_remote(&self, session: &Session, remote: Option<(&str, u16)>) {
        self.set_session_outbound_with_path(session, remote, Vec::new());
    }

    pub fn set_session_outbound_with_path(
        &self,
        session: &Session,
        remote: Option<(&str, u16)>,
        relay_chain: Vec<(String, String)>,
    ) {
        let outbound_protocol = session
            .outbound_tag
            .as_deref()
            .and_then(|tag| self.outbound_protocol_for_tag(tag));
        let active = self.session_registry.update_outbound(
            session.id,
            session.outbound_tag.as_deref(),
            outbound_protocol,
            remote.map(|(host, port)| FlowRemoteEndpoint {
                host: host.to_owned(),
                port,
            }),
            relay_chain,
        );
        if let Some(active) = active {
            self.event_log.push_flow_routed(&active);
        }
    }

    pub fn record_session_route(&self, id: u64, trace: &RouteTrace) {
        let (action, target) = match &trace.decision {
            RouteDecision::Route(tag) => ("route".to_owned(), Some(tag.clone())),
            RouteDecision::Direct => ("direct".to_owned(), None),
            RouteDecision::Reject => ("reject".to_owned(), None),
        };
        let selection_chain = target.iter().cloned().collect();
        self.session_registry.update_route(
            id,
            FlowRouteObservation {
                mode: trace.mode.clone(),
                action,
                target,
                matched_rule: trace.matched_rule.clone(),
                selection_chain,
            },
        );
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
        self.finish_session_with_observation(id, outcome, reason, None)
    }

    pub fn finish_session_with_observation(
        &self,
        id: u64,
        outcome: SessionOutcome,
        reason: Option<String>,
        failure: Option<FlowFailureObservation>,
    ) -> Option<CompletedSessionRecord> {
        let record = self.session_registry.finish(id, outcome, reason, failure)?;
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
            .map(|outbound| outbound.protocol.protocol_name())
    }
}
