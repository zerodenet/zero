use crate::runtime::udp_flow::result::FlowFailure;
use zero_engine::EngineError;

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
