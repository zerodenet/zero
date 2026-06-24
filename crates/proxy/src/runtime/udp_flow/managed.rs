use std::collections::HashMap;

use zero_core::{Address, Session};
use zero_engine::{SessionHandle, SessionOutcome};

use crate::logging::log_session_finished;

#[derive(Debug, Default)]
pub(crate) struct ManagedUdpFlows {
    handles: HashMap<(Address, u16), (Session, SessionHandle)>,
}

impl ManagedUdpFlows {
    pub(crate) fn insert(&mut self, session: Session, handle: SessionHandle) {
        self.handles
            .insert((session.target.clone(), session.port), (session, handle));
    }

    pub(crate) fn finish_all(&mut self) {
        for (_key, (session, mut handle)) in self.handles.drain() {
            if let Some(record) = handle.finish(SessionOutcome::ChainedRelayed) {
                log_session_finished(&record, None);
                let _ = session;
            }
        }
    }
}
