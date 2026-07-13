pub(crate) fn upstream_flow_mismatch(
    stage: &'static str,
    server: &str,
    port: u16,
    message: &'static str,
) -> crate::runtime::udp_flow::result::FlowFailure {
    crate::runtime::udp_flow::result::FlowFailure {
        stage,
        error: zero_engine::EngineError::Io(std::io::Error::other(message)),
        upstream: Some((server.to_string(), port)),
    }
}
