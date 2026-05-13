use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::Value;
use zero_api::{
    event_type, ApiEvent, AuthInfo, EndpointRef, EventFilter, FlowEventPayload, FlowOutcome,
    FlowTiming, Network as ApiNetwork, PolicyDecision, RawApiEvent, RouteDecision, TargetAddress,
    TrafficStats,
};
use zero_core::{Address, Network, ProtocolType};

use super::completed_sessions::CompletedSessionRecord;
use super::stats::SessionOutcome;

const DEFAULT_EVENT_LOG_CAPACITY: usize = 1024;

#[derive(Debug)]
pub struct EngineEventLog {
    capacity: usize,
    next_sequence: AtomicU64,
    inner: Mutex<VecDeque<RawApiEvent>>,
}

impl Default for EngineEventLog {
    fn default() -> Self {
        Self {
            capacity: DEFAULT_EVENT_LOG_CAPACITY,
            next_sequence: AtomicU64::new(1),
            inner: Mutex::new(VecDeque::with_capacity(DEFAULT_EVENT_LOG_CAPACITY)),
        }
    }
}

impl EngineEventLog {
    pub fn shared() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn push_flow_completed(
        &self,
        record: &CompletedSessionRecord,
        outbound_protocol: impl FnOnce(&str) -> Option<&'static str>,
    ) {
        let event = flow_completed_event(record, outbound_protocol);
        self.push(event);
    }

    pub fn snapshot(&self, filter: &EventFilter) -> Vec<RawApiEvent> {
        self.inner
            .lock()
            .expect("engine event log lock poisoned")
            .iter()
            .filter(|event| matches_filter(event, filter))
            .cloned()
            .collect()
    }

    fn push(&self, mut event: RawApiEvent) {
        let sequence = self.next_sequence.fetch_add(1, Ordering::Relaxed);
        event.sequence = Some(sequence);

        let mut events = self.inner.lock().expect("engine event log lock poisoned");
        events.push_back(event);

        while events.len() > self.capacity {
            events.pop_front();
        }
    }
}

fn flow_completed_event(
    record: &CompletedSessionRecord,
    outbound_protocol: impl FnOnce(&str) -> Option<&'static str>,
) -> RawApiEvent {
    let auth = record.auth.as_ref().map(|auth| AuthInfo {
        scheme: auth.scheme.clone(),
        credential_id: auth.credential_id.clone(),
        principal_key: auth.principal_key.clone(),
        attributes: BTreeMap::new(),
    });
    let principal_key = auth.as_ref().and_then(|auth| auth.principal_key.clone());

    let payload = FlowEventPayload {
        flow_id: record.id.to_string(),
        network: api_network(record.network),
        inbound: EndpointRef {
            tag: record.inbound_tag.clone().unwrap_or_default(),
            protocol: protocol_name(record.protocol).to_owned(),
        },
        auth,
        target: TargetAddress {
            host: address_host(&record.target),
            port: record.port,
        },
        route: RouteDecision {
            mode: record.mode.clone(),
            target: None,
        },
        policy: None::<PolicyDecision>,
        outbound: record.outbound_tag.as_ref().map(|tag| EndpointRef {
            tag: tag.clone(),
            protocol: outbound_protocol(tag).unwrap_or("unknown").to_owned(),
        }),
        traffic: TrafficStats {
            bytes_up: record.bytes_up,
            bytes_down: record.bytes_down,
            inbound_rx_bytes: Some(record.inbound_rx_bytes),
            inbound_tx_bytes: Some(record.inbound_tx_bytes),
            outbound_rx_bytes: Some(record.outbound_rx_bytes),
            outbound_tx_bytes: Some(record.outbound_tx_bytes),
            packets_up: None,
            packets_down: None,
        },
        timing: FlowTiming {
            started_at_unix_ms: record.started_at_unix_ms,
            ended_at_unix_ms: Some(record.finished_at_unix_ms),
            duration_ms: Some(record.duration_ms),
        },
        outcome: api_outcome(record.outcome),
    };

    let payload =
        serde_json::to_value(payload).expect("flow completed event payload should be serializable");
    let mut event = ApiEvent::new(
        format!(
            "{}:{}:{}",
            event_type::FLOW_COMPLETED,
            record.id,
            record.finished_at_unix_ms
        ),
        event_type::FLOW_COMPLETED,
        record.finished_at_unix_ms,
        payload,
    );
    event.labels = BTreeMap::new();
    event.principal_key = principal_key;
    event
}

fn matches_filter(event: &RawApiEvent, filter: &EventFilter) -> bool {
    if !filter.event_types.is_empty()
        && !filter
            .event_types
            .iter()
            .any(|event_type| event_type == &event.event_type)
    {
        return false;
    }

    if !filter.principal_keys.is_empty() {
        let principal_key = event.principal_key.as_deref();
        if !filter
            .principal_keys
            .iter()
            .any(|expected| Some(expected.as_str()) == principal_key)
        {
            return false;
        }
    }

    if !filter.inbound_tags.is_empty() {
        let inbound_tag = payload_inbound_tag(&event.payload);
        if !filter
            .inbound_tags
            .iter()
            .any(|expected| Some(expected.as_str()) == inbound_tag)
        {
            return false;
        }
    }

    true
}

fn payload_inbound_tag(payload: &Value) -> Option<&str> {
    payload.get("inbound")?.get("tag")?.as_str()
}

fn api_network(network: Network) -> ApiNetwork {
    match network {
        Network::Tcp => ApiNetwork::Tcp,
        Network::Udp => ApiNetwork::Udp,
    }
}

fn api_outcome(outcome: SessionOutcome) -> FlowOutcome {
    match outcome {
        SessionOutcome::DirectRelayed => FlowOutcome::DirectRelayed,
        SessionOutcome::ChainedRelayed => FlowOutcome::ChainedRelayed,
        SessionOutcome::Blocked => FlowOutcome::Blocked,
        SessionOutcome::Failed => FlowOutcome::Failed,
    }
}

fn protocol_name(protocol: ProtocolType) -> &'static str {
    match protocol {
        ProtocolType::Socks5 => "socks5",
        ProtocolType::HttpConnect => "http-connect",
        ProtocolType::Vless => "vless",
        ProtocolType::Hysteria2 => "hysteria2",
        ProtocolType::Unknown => "unknown",
    }
}

fn address_host(address: &Address) -> String {
    match address {
        Address::Domain(domain) => domain.clone(),
        Address::Ipv4(addr) => format!("{}.{}.{}.{}", addr[0], addr[1], addr[2], addr[3]),
        Address::Ipv6(addr) => std::net::Ipv6Addr::from(*addr).to_string(),
    }
}
