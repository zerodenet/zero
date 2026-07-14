use std::sync::mpsc::SyncSender;

use zero_api::{EventFilter, RawApiEvent};

use super::Engine;
use crate::{ActiveSession, CompletedSessionRecord, EventsSinceResult};

impl Engine {
    pub(crate) fn subscribe_events(&self, subscriber: SyncSender<RawApiEvent>) {
        self.event_log.subscribe(subscriber);
    }

    pub(crate) fn emit_event(&self, event: RawApiEvent) {
        self.event_log.push_external(event);
    }

    pub fn stats_snapshot(&self) -> zero_api::StatsSnapshot {
        self.stats.snapshot()
    }

    pub fn active_sessions(&self) -> Vec<ActiveSession> {
        self.session_registry.snapshot()
    }

    pub fn completed_sessions(&self) -> Vec<CompletedSessionRecord> {
        self.completed_sessions.snapshot()
    }

    pub fn events_snapshot(&self, filter: &EventFilter) -> Vec<RawApiEvent> {
        self.event_log.snapshot(filter)
    }

    pub fn events_since(
        &self,
        since: u64,
        limit: usize,
        filter: &EventFilter,
    ) -> EventsSinceResult {
        self.event_log.events_since(since, limit, filter)
    }

    pub fn latest_event_sequence(&self) -> u64 {
        self.event_log.latest_sequence()
    }

    pub fn push_stats_sampled(&self) {
        self.event_log.push_stats_sampled(&self.stats_snapshot());
    }

    pub fn push_engine_stopped(&self, reason: &str) {
        self.event_log.push_engine_stopped(reason);
    }

    pub fn update_sink_status(&self, status: Vec<zero_api::SinkStatus>) {
        *self.sink_status.lock().expect("sink status lock poisoned") = status;
    }

    pub fn sink_status_snapshot(&self) -> zero_api::SinkStatusSnapshot {
        zero_api::SinkStatusSnapshot {
            sinks: self
                .sink_status
                .lock()
                .expect("sink status lock poisoned")
                .clone(),
        }
    }
}
