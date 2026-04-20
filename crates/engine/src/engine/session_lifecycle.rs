use super::completed_sessions::CompletedSessionRecord;
use super::runtime::Engine;
use super::stats::SessionOutcome;

#[derive(Debug)]
pub struct SessionHandle {
    engine: Engine,
    session_id: u64,
    finished: bool,
}

impl SessionHandle {
    pub fn new(engine: Engine, session_id: u64) -> Self {
        Self {
            engine,
            session_id,
            finished: false,
        }
    }

    pub fn finish(&mut self, outcome: SessionOutcome) -> Option<CompletedSessionRecord> {
        if self.finished {
            return None;
        }

        let record = self.engine.finish_session(self.session_id, outcome);
        self.finished = true;
        record
    }
}

impl Drop for SessionHandle {
    fn drop(&mut self) {
        if !self.finished {
            let _ = self
                .engine
                .finish_session(self.session_id, SessionOutcome::Failed);
        }
    }
}
