use zero_engine::EngineError;
#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
use zero_engine::ResolvedLeafOutbound;
#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
use zero_transport::outbound_leaf::ResolveTransportLeafError;

#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
use super::model::TcpOutboundFailure;

/// Detect the synthetic blocked result normalized by TCP outbound dispatch.
pub(crate) fn is_block_error(error: &EngineError) -> bool {
    matches!(
        error,
        EngineError::Io(error)
            if error.kind() == std::io::ErrorKind::ConnectionRefused
                && error.to_string() == "blocked"
    )
}

#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
pub(super) fn invalid_input_error(
    stage: &'static str,
    error: impl std::fmt::Display,
) -> EngineError {
    EngineError::Io(std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        format!("{stage}: {error}"),
    ))
}

#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
pub(super) fn prefixed_expected_outbound_leaf_error(
    stage: &'static str,
    message: &'static str,
) -> EngineError {
    invalid_input_error(stage, message)
}

#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
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

#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
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

#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
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
