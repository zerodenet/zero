use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use serde::Serialize;

#[derive(Debug, Default)]
pub struct EngineStats {
    total_started: AtomicU64,
    active_sessions: AtomicU64,
    completed_sessions: AtomicU64,
    failed_sessions: AtomicU64,
    blocked_sessions: AtomicU64,
    direct_sessions: AtomicU64,
    chained_sessions: AtomicU64,
}

impl EngineStats {
    pub fn shared() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn snapshot(&self) -> EngineStatsSnapshot {
        EngineStatsSnapshot {
            total_started: self.total_started.load(Ordering::Relaxed),
            active_sessions: self.active_sessions.load(Ordering::Relaxed),
            completed_sessions: self.completed_sessions.load(Ordering::Relaxed),
            failed_sessions: self.failed_sessions.load(Ordering::Relaxed),
            blocked_sessions: self.blocked_sessions.load(Ordering::Relaxed),
            direct_sessions: self.direct_sessions.load(Ordering::Relaxed),
            chained_sessions: self.chained_sessions.load(Ordering::Relaxed),
        }
    }

    pub fn record_start(&self) {
        self.total_started.fetch_add(1, Ordering::Relaxed);
        self.active_sessions.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_finish(&self, outcome: SessionOutcome) {
        self.active_sessions.fetch_sub(1, Ordering::Relaxed);

        match outcome {
            SessionOutcome::DirectRelayed => {
                self.completed_sessions.fetch_add(1, Ordering::Relaxed);
                self.direct_sessions.fetch_add(1, Ordering::Relaxed);
            }
            SessionOutcome::ChainedRelayed => {
                self.completed_sessions.fetch_add(1, Ordering::Relaxed);
                self.chained_sessions.fetch_add(1, Ordering::Relaxed);
            }
            SessionOutcome::Blocked => {
                self.blocked_sessions.fetch_add(1, Ordering::Relaxed);
            }
            SessionOutcome::Failed => {
                self.failed_sessions.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionOutcome {
    DirectRelayed,
    ChainedRelayed,
    Blocked,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct EngineStatsSnapshot {
    pub total_started: u64,
    pub active_sessions: u64,
    pub completed_sessions: u64,
    pub failed_sessions: u64,
    pub blocked_sessions: u64,
    pub direct_sessions: u64,
    pub chained_sessions: u64,
}
