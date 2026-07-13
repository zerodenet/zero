use zero_engine::{EngineError, ResolvedLeafOutbound};
use zero_transport::outbound_leaf::ResolveTransportLeafError;

use crate::runtime::udp_flow::result::FlowFailure;
#[cfg(feature = "vless")]
use crate::transport::TcpOutboundFailure;

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

pub(super) fn udp_flow_failure(
    stage: &'static str,
    error: EngineError,
    upstream: Option<(&str, u16)>,
) -> FlowFailure {
    FlowFailure {
        stage,
        error,
        upstream: upstream.map(|(server, port)| (server.to_string(), port)),
    }
}

#[cfg(feature = "vless")]
pub(super) fn relay_chain_flow_failure(failure: TcpOutboundFailure) -> FlowFailure {
    FlowFailure {
        stage: failure.stage,
        error: failure.error,
        upstream: failure.upstream_endpoint,
    }
}

pub(super) fn udp_prepare_failure<E>(
    leaf: &ResolvedLeafOutbound<'_>,
    error: ResolveTransportLeafError<E>,
    stage: &'static str,
    invalid_config: &'static str,
    expected_leaf: &'static str,
) -> FlowFailure
where
    E: std::fmt::Display,
{
    let upstream = leaf.proxy_endpoint();
    match error {
        ResolveTransportLeafError::InvalidConfig(error) => {
            udp_flow_failure(stage, invalid_input_error(invalid_config, error), upstream)
        }
        ResolveTransportLeafError::MissingLeaf => {
            udp_flow_failure(stage, expected_outbound_leaf_error(expected_leaf), None)
        }
    }
}

#[cfg(feature = "vless")]
pub(super) fn last_udp_prepare_failure<E>(
    chain: &[ResolvedLeafOutbound<'_>],
    error: ResolveTransportLeafError<E>,
    stage: &'static str,
    invalid_config: &'static str,
    expected_leaf: &'static str,
) -> FlowFailure
where
    E: std::fmt::Display,
{
    match error {
        ResolveTransportLeafError::InvalidConfig(error) => {
            let upstream = chain.last().and_then(|leaf| leaf.proxy_endpoint());
            udp_flow_failure(stage, invalid_input_error(invalid_config, error), upstream)
        }
        ResolveTransportLeafError::MissingLeaf => {
            udp_flow_failure(stage, expected_outbound_leaf_error(expected_leaf), None)
        }
    }
}
