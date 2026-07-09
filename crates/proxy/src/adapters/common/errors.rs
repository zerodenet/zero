use zero_engine::EngineError;

use crate::runtime::udp_dispatch::FlowFailure;
use crate::transport::TcpOutboundFailure;

fn owned_upstream_endpoint((server, port): (&str, u16)) -> (String, u16) {
    (server.to_string(), port)
}

pub(super) fn invalid_input_error(
    stage: &'static str,
    error: impl std::fmt::Display,
) -> EngineError {
    EngineError::Io(std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        format!("{stage}: {error}"),
    ))
}

pub(super) fn expected_outbound_leaf_error(message: &'static str) -> EngineError {
    EngineError::Io(std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        message,
    ))
}

pub(super) fn prefixed_expected_outbound_leaf_error(
    stage: &'static str,
    message: &'static str,
) -> EngineError {
    invalid_input_error(stage, message)
}

pub(super) fn tcp_failure(
    stage: &'static str,
    error: EngineError,
    upstream: Option<(&str, u16)>,
) -> TcpOutboundFailure {
    TcpOutboundFailure {
        stage,
        error,
        upstream_endpoint: upstream.map(owned_upstream_endpoint),
    }
}

pub(super) fn udp_flow_failure(
    stage: &'static str,
    error: EngineError,
    upstream: Option<(&str, u16)>,
) -> FlowFailure {
    FlowFailure {
        stage,
        error,
        upstream: upstream.map(owned_upstream_endpoint),
    }
}

pub(super) fn relay_chain_flow_failure(failure: TcpOutboundFailure) -> FlowFailure {
    FlowFailure {
        stage: failure.stage,
        error: failure.error,
        upstream: failure.upstream_endpoint,
    }
}
