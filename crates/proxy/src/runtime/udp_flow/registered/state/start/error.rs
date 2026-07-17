#[cfg(feature = "udp-runtime")]
use zero_engine::EngineError;

#[cfg(feature = "udp-runtime")]
use crate::runtime::udp_flow::result::FlowFailure;

#[cfg(feature = "udp-runtime")]
pub(super) fn unhandled_managed_flow() -> FlowFailure {
    FlowFailure {
        stage: "udp_managed_flow_start",
        error: EngineError::Io(std::io::Error::other(
            "managed UDP flow request had no compiled start handler",
        )),
        upstream: None,
    }
}
