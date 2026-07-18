use zero_engine::{CompletedSessionRecord, EngineError, PassiveRelayOutcome};

const EARLY_RELAY_FAILURE_LIMIT_MS: u64 = 3_000;

pub(super) fn classify_relay_outcome(
    record: &CompletedSessionRecord,
    error: Option<&EngineError>,
) -> PassiveRelayOutcome {
    if record.outbound_rx_bytes > 0 {
        return PassiveRelayOutcome::Success;
    }

    if record.duration_ms <= EARLY_RELAY_FAILURE_LIMIT_MS
        && record.outbound_tx_bytes > 0
        && error.is_some_and(is_early_transport_failure)
    {
        return PassiveRelayOutcome::Failure;
    }

    PassiveRelayOutcome::Neutral
}

fn is_early_transport_failure(error: &EngineError) -> bool {
    let message = error.to_string().to_ascii_lowercase();
    message.contains("unexpected eof")
        || message.contains("broken pipe")
        || message.contains("connection reset")
        || message.contains("forcibly closed")
        || message.contains("os error 10054")
}

#[cfg(test)]
mod tests {
    use std::io;

    use zero_core::{Address, Network, ProtocolType};
    use zero_engine::SessionOutcome;

    use super::*;

    fn record(
        duration_ms: u64,
        outbound_tx_bytes: u64,
        outbound_rx_bytes: u64,
    ) -> CompletedSessionRecord {
        CompletedSessionRecord {
            id: 1,
            inbound_tag: Some("entry".to_owned()),
            outbound_tag: Some("hk-ss-1".to_owned()),
            target: Address::Domain("landing.example".to_owned()),
            port: 14788,
            protocol: ProtocolType::UNKNOWN,
            auth: None,
            network: Network::Tcp,
            mode: "rule".to_owned(),
            started_at_unix_ms: 0,
            last_activity_at_unix_ms: 0,
            finished_at_unix_ms: duration_ms,
            duration_ms,
            bytes_up: outbound_tx_bytes,
            bytes_down: outbound_rx_bytes,
            inbound_rx_bytes: outbound_tx_bytes,
            inbound_tx_bytes: outbound_rx_bytes,
            outbound_rx_bytes,
            outbound_tx_bytes,
            process_id: None,
            process_name: None,
            outcome: SessionOutcome::Failed,
            close_reason: Some("upstream_error".to_owned()),
        }
    }

    #[test]
    fn classifies_the_observed_shadowsocks_failure() {
        let error = EngineError::Io(io::Error::other("shadowsocks unexpected EOF"));
        assert_eq!(
            classify_relay_outcome(&record(459, 1749, 0), Some(&error)),
            PassiveRelayOutcome::Failure
        );
    }

    #[test]
    fn does_not_penalize_a_flow_that_received_upstream_data() {
        let error = EngineError::Io(io::Error::new(io::ErrorKind::BrokenPipe, "broken pipe"));
        assert_eq!(
            classify_relay_outcome(&record(459, 1749, 1), Some(&error)),
            PassiveRelayOutcome::Success
        );
    }

    #[test]
    fn does_not_penalize_late_or_unclassified_failures() {
        let eof = EngineError::Io(io::Error::other("shadowsocks unexpected EOF"));
        let other = EngineError::Io(io::Error::other("application rejected request"));
        assert_eq!(
            classify_relay_outcome(&record(3_001, 1749, 0), Some(&eof)),
            PassiveRelayOutcome::Neutral
        );
        assert_eq!(
            classify_relay_outcome(&record(459, 1749, 0), Some(&other)),
            PassiveRelayOutcome::Neutral
        );
    }
}
