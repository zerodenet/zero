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

    pub fn finish(&mut self, outcome: SessionOutcome) {
        if self.finished {
            return;
        }

        self.engine.finish_session(self.session_id, outcome);
        self.finished = true;
    }
}

impl Drop for SessionHandle {
    fn drop(&mut self) {
        if !self.finished {
            self.engine
                .finish_session(self.session_id, SessionOutcome::Failed);
        }
    }
}
