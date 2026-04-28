use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use zero_core::{Address, Network, ProtocolType, Session, SessionAuth};

use super::completed_sessions::CompletedSessionRecord;
use super::stats::SessionOutcome;
use super::traffic_sampler::TrafficSampler;

#[derive(Debug, Default)]
pub struct SessionRegistry {
    inner: Mutex<HashMap<u64, Arc<ActiveSessionEntry>>>,
}

impl SessionRegistry {
    pub fn shared() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn insert(&self, session: &Session, mode: &str) {
        let active = Arc::new(ActiveSessionEntry::new(session, mode));
        self.inner
            .lock()
            .expect("session registry lock poisoned")
            .insert(active.id, active);
    }

    pub fn update_outbound_tag(&self, session_id: u64, outbound_tag: Option<&str>) {
        if let Some(session) = self.get(session_id) {
            session.update_outbound_tag(outbound_tag);
        }
    }

    pub fn record_upload(&self, session_id: u64, bytes: u64) {
        if let Some(session) = self.get(session_id) {
            session.record_upload(bytes);
        }
    }

    pub fn record_download(&self, session_id: u64, bytes: u64) {
        if let Some(session) = self.get(session_id) {
            session.record_download(bytes);
        }
    }

    pub fn record_inbound_rx(&self, session_id: u64, bytes: u64) {
        if let Some(session) = self.get(session_id) {
            session.record_inbound_rx(bytes);
        }
    }

    pub fn record_inbound_tx(&self, session_id: u64, bytes: u64) {
        if let Some(session) = self.get(session_id) {
            session.record_inbound_tx(bytes);
        }
    }

    pub fn record_outbound_rx(&self, session_id: u64, bytes: u64) {
        if let Some(session) = self.get(session_id) {
            session.record_outbound_rx(bytes);
        }
    }

    pub fn record_outbound_tx(&self, session_id: u64, bytes: u64) {
        if let Some(session) = self.get(session_id) {
            session.record_outbound_tx(bytes);
        }
    }

    pub fn finish(
        &self,
        session_id: u64,
        outcome: SessionOutcome,
    ) -> Option<CompletedSessionRecord> {
        self.inner
            .lock()
            .expect("session registry lock poisoned")
            .remove(&session_id)
            .map(|session| session.finish(outcome))
    }

    pub fn snapshot(&self) -> Vec<ActiveSession> {
        let mut sessions = self
            .inner
            .lock()
            .expect("session registry lock poisoned")
            .values()
            .map(|session| session.snapshot())
            .collect::<Vec<_>>();
        sessions.sort_by_key(|session| session.id);
        sessions
    }

    fn get(&self, session_id: u64) -> Option<Arc<ActiveSessionEntry>> {
        self.inner
            .lock()
            .expect("session registry lock poisoned")
            .get(&session_id)
            .cloned()
    }
}

#[derive(Debug)]
struct ActiveSessionEntry {
    id: u64,
    inbound_tag: Option<String>,
    outbound_tag: Mutex<Option<String>>,
    target: Address,
    port: u16,
    protocol: ProtocolType,
    auth: Option<SessionAuth>,
    network: Network,
    mode: String,
    started_at: Instant,
    started_at_unix_ms: u64,
    last_activity_at_unix_ms: AtomicU64,
    bytes_up: AtomicU64,
    bytes_down: AtomicU64,
    inbound_rx_bytes: AtomicU64,
    inbound_tx_bytes: AtomicU64,
    outbound_rx_bytes: AtomicU64,
    outbound_tx_bytes: AtomicU64,
    throughput_sampler: TrafficSampler,
}

impl ActiveSessionEntry {
    fn new(session: &Session, mode: &str) -> Self {
        let started_at_unix_ms = unix_timestamp_ms();

        Self {
            id: session.id,
            inbound_tag: session.inbound_tag.clone(),
            outbound_tag: Mutex::new(session.outbound_tag.clone()),
            target: session.target.clone(),
            port: session.port,
            protocol: session.protocol,
            auth: session.auth.clone(),
            network: session.network,
            mode: mode.to_owned(),
            started_at: Instant::now(),
            started_at_unix_ms,
            last_activity_at_unix_ms: AtomicU64::new(started_at_unix_ms),
            bytes_up: AtomicU64::new(0),
            bytes_down: AtomicU64::new(0),
            inbound_rx_bytes: AtomicU64::new(0),
            inbound_tx_bytes: AtomicU64::new(0),
            outbound_rx_bytes: AtomicU64::new(0),
            outbound_tx_bytes: AtomicU64::new(0),
            throughput_sampler: TrafficSampler::new(started_at_unix_ms),
        }
    }

    fn update_outbound_tag(&self, outbound_tag: Option<&str>) {
        *self
            .outbound_tag
            .lock()
            .expect("outbound tag lock poisoned") = outbound_tag.map(ToOwned::to_owned);
    }

    fn record_upload(&self, bytes: u64) {
        self.record_inbound_rx(bytes);
        self.record_outbound_tx(bytes);
    }

    fn record_download(&self, bytes: u64) {
        self.record_outbound_rx(bytes);
        self.record_inbound_tx(bytes);
    }

    fn record_inbound_rx(&self, bytes: u64) {
        if bytes == 0 {
            return;
        }

        self.bytes_up.fetch_add(bytes, Ordering::Relaxed);
        self.inbound_rx_bytes.fetch_add(bytes, Ordering::Relaxed);
        self.touch();
    }

    fn record_inbound_tx(&self, bytes: u64) {
        if bytes == 0 {
            return;
        }

        self.bytes_down.fetch_add(bytes, Ordering::Relaxed);
        self.inbound_tx_bytes.fetch_add(bytes, Ordering::Relaxed);
        self.touch();
    }

    fn record_outbound_rx(&self, bytes: u64) {
        if bytes == 0 {
            return;
        }

        self.bytes_down.fetch_add(bytes, Ordering::Relaxed);
        self.outbound_rx_bytes.fetch_add(bytes, Ordering::Relaxed);
        self.touch();
    }

    fn record_outbound_tx(&self, bytes: u64) {
        if bytes == 0 {
            return;
        }

        self.bytes_up.fetch_add(bytes, Ordering::Relaxed);
        self.outbound_tx_bytes.fetch_add(bytes, Ordering::Relaxed);
        self.touch();
    }

    fn snapshot(&self) -> ActiveSession {
        let now_unix_ms = unix_timestamp_ms();
        let bytes_up = self.bytes_up.load(Ordering::Relaxed);
        let bytes_down = self.bytes_down.load(Ordering::Relaxed);
        let throughput = self
            .throughput_sampler
            .snapshot(now_unix_ms, bytes_up, bytes_down);

        ActiveSession {
            id: self.id,
            inbound_tag: self.inbound_tag.clone(),
            outbound_tag: self
                .outbound_tag
                .lock()
                .expect("outbound tag lock poisoned")
                .clone(),
            target: self.target.clone(),
            port: self.port,
            protocol: self.protocol,
            auth: self.auth.clone(),
            network: self.network,
            mode: self.mode.clone(),
            started_at_unix_ms: self.started_at_unix_ms,
            last_activity_at_unix_ms: self.last_activity_at_unix_ms.load(Ordering::Relaxed),
            bytes_up,
            bytes_down,
            inbound_rx_bytes: self.inbound_rx_bytes.load(Ordering::Relaxed),
            inbound_tx_bytes: self.inbound_tx_bytes.load(Ordering::Relaxed),
            outbound_rx_bytes: self.outbound_rx_bytes.load(Ordering::Relaxed),
            outbound_tx_bytes: self.outbound_tx_bytes.load(Ordering::Relaxed),
            throughput_up_bps: throughput.up_bps,
            throughput_down_bps: throughput.down_bps,
            process_id: None,
            process_name: None,
        }
    }

    fn finish(&self, outcome: SessionOutcome) -> CompletedSessionRecord {
        let finished_at_unix_ms = unix_timestamp_ms();
        let duration_ms = self.started_at.elapsed().as_millis() as u64;
        let bytes_up = self.bytes_up.load(Ordering::Relaxed);
        let bytes_down = self.bytes_down.load(Ordering::Relaxed);

        CompletedSessionRecord {
            id: self.id,
            inbound_tag: self.inbound_tag.clone(),
            outbound_tag: self
                .outbound_tag
                .lock()
                .expect("outbound tag lock poisoned")
                .clone(),
            target: self.target.clone(),
            port: self.port,
            protocol: self.protocol,
            auth: self.auth.clone(),
            network: self.network,
            mode: self.mode.clone(),
            started_at_unix_ms: self.started_at_unix_ms,
            last_activity_at_unix_ms: self.last_activity_at_unix_ms.load(Ordering::Relaxed),
            finished_at_unix_ms,
            duration_ms,
            bytes_up,
            bytes_down,
            inbound_rx_bytes: self.inbound_rx_bytes.load(Ordering::Relaxed),
            inbound_tx_bytes: self.inbound_tx_bytes.load(Ordering::Relaxed),
            outbound_rx_bytes: self.outbound_rx_bytes.load(Ordering::Relaxed),
            outbound_tx_bytes: self.outbound_tx_bytes.load(Ordering::Relaxed),
            process_id: None,
            process_name: None,
            outcome,
        }
    }

    fn touch(&self) {
        self.last_activity_at_unix_ms
            .store(unix_timestamp_ms(), Ordering::Relaxed);
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
    pub auth: Option<SessionAuth>,
    pub network: Network,
    pub mode: String,
    pub started_at_unix_ms: u64,
    pub last_activity_at_unix_ms: u64,
    pub bytes_up: u64,
    pub bytes_down: u64,
    pub inbound_rx_bytes: u64,
    pub inbound_tx_bytes: u64,
    pub outbound_rx_bytes: u64,
    pub outbound_tx_bytes: u64,
    pub throughput_up_bps: u64,
    pub throughput_down_bps: u64,
    pub process_id: Option<u32>,
    pub process_name: Option<String>,
}

fn unix_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_millis() as u64
}
