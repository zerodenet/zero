use zero_engine::EngineError;

#[cfg(feature = "udp-runtime")]
use crate::runtime::udp_dispatch::FlowFailure;

fn unsupported_io(message: &'static str) -> EngineError {
    EngineError::Io(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        message,
    ))
}

pub(in crate::protocol_registry) fn relay_hop_unsupported() -> EngineError {
    unsupported_io("this adapter does not support relay hop")
}

#[cfg(feature = "udp-runtime")]

pub(in crate::protocol_registry) fn udp_relay_final_hop_unsupported() -> FlowFailure {
    udp_flow_unsupported(
        "no_udp_relay_final_hop",
        "this adapter does not support UDP relay final hop",
    )
}

#[cfg(feature = "udp-runtime")]

fn udp_flow_unsupported(stage: &'static str, message: &'static str) -> FlowFailure {
    FlowFailure {
        stage,
        error: unsupported_io(message),
        upstream: None,
    }
}
