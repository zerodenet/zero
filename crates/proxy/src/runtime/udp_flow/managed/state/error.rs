use crate::runtime::udp_dispatch::FlowFailure;
use zero_engine::EngineError;

pub(super) fn managed_forward_unavailable(
    stage: &'static str,
    message: &'static str,
) -> FlowFailure {
    FlowFailure {
        stage,
        error: EngineError::Io(std::io::Error::other(message)),
        upstream: None,
    }
}

pub(in crate::runtime::udp_flow::managed) fn flow_mismatch(
    stage: &'static str,
    server: &str,
    port: u16,
    message: &'static str,
) -> FlowFailure {
    FlowFailure {
        stage,
        error: EngineError::Io(std::io::Error::other(message)),
        upstream: Some((server.to_string(), port)),
    }
}
