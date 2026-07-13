use zero_engine::EngineError;
/// Detect the synthetic blocked result normalized by TCP outbound dispatch.
pub(crate) fn is_block_error(error: &EngineError) -> bool {
    matches!(
        error,
        EngineError::Io(error)
            if error.kind() == std::io::ErrorKind::ConnectionRefused
                && error.to_string() == "blocked"
    )
}
