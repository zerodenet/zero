use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use zero_core::{Address, ProtocolType, Session};

#[derive(Debug, Default)]
pub struct SessionRegistry {
    inner: Mutex<HashMap<u64, ActiveSession>>,
}

impl SessionRegistry {
    pub fn shared() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn insert(&self, session: &Session) {
        let active = ActiveSession::from(session);
        self.inner
            .lock()
            .expect("session registry lock poisoned")
            .insert(active.id, active);
    }

    pub fn update_outbound_tag(&self, session_id: u64, outbound_tag: Option<&str>) {
        if let Some(session) = self
            .inner
            .lock()
            .expect("session registry lock poisoned")
            .get_mut(&session_id)
        {
            session.outbound_tag = outbound_tag.map(ToOwned::to_owned);
        }
    }

    pub fn remove(&self, session_id: u64) {
        self.inner
            .lock()
            .expect("session registry lock poisoned")
            .remove(&session_id);
    }

    pub fn snapshot(&self) -> Vec<ActiveSession> {
        let mut sessions = self
            .inner
            .lock()
            .expect("session registry lock poisoned")
            .values()
            .cloned()
            .collect::<Vec<_>>();
        sessions.sort_by_key(|session| session.id);
        sessions
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveSession {
    pub id: u64,
    pub inbound_tag: Option<String>,
    pub outbound_tag: Option<String>,
    pub target: Address,
    pub port: u16,
    pub protocol: ProtocolType,
}

impl From<&Session> for ActiveSession {
    fn from(session: &Session) -> Self {
        Self {
            id: session.id,
            inbound_tag: session.inbound_tag.clone(),
            outbound_tag: session.outbound_tag.clone(),
            target: session.target.clone(),
            port: session.port,
            protocol: session.protocol,
        }
    }
}
