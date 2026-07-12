use zero_engine::{EngineError, ResolvedLeafOutbound};
use zero_transport::outbound_leaf::ResolveTransportLeafError;

use super::model::TcpOutboundFailure;

pub(super) fn invalid_input_error(
    stage: &'static str,
    error: impl std::fmt::Display,
) -> EngineError {
    EngineError::Io(std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        format!("{stage}: {error}"),
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
        upstream_endpoint: upstream.map(|(server, port)| (server.to_string(), port)),
    }
}

pub(super) fn tcp_connect_prepare_failure<E>(
    leaf: &ResolvedLeafOutbound<'_>,
    error: ResolveTransportLeafError<E>,
    stage: &'static str,
    invalid_config: &'static str,
    invalid_leaf_stage: &'static str,
    expected_leaf: &'static str,
) -> TcpOutboundFailure
where
    E: std::fmt::Display,
{
    let upstream = leaf.proxy_endpoint();
    match error {
        ResolveTransportLeafError::InvalidConfig(error) => {
            tcp_failure(stage, invalid_input_error(invalid_config, error), upstream)
        }
        ResolveTransportLeafError::MissingLeaf => tcp_failure(
            stage,
            prefixed_expected_outbound_leaf_error(invalid_leaf_stage, expected_leaf),
            None,
        ),
    }
}

pub(super) fn tcp_relay_prepare_error<E>(
    error: ResolveTransportLeafError<E>,
    invalid_config: &'static str,
    invalid_leaf_stage: &'static str,
    expected_leaf: &'static str,
) -> EngineError
where
    E: std::fmt::Display,
{
    match error {
        ResolveTransportLeafError::InvalidConfig(error) => {
            invalid_input_error(invalid_config, error)
        }
        ResolveTransportLeafError::MissingLeaf => {
            prefixed_expected_outbound_leaf_error(invalid_leaf_stage, expected_leaf)
        }
    }
}
