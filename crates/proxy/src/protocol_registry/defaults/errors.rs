use zero_engine::{EngineError, ResolvedLeafOutbound};

use crate::runtime::udp_dispatch::FlowFailure;
use crate::transport::TcpOutboundFailure;

fn unsupported_io(message: &'static str) -> EngineError {
    EngineError::Io(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        message,
    ))
}

pub(in crate::protocol_registry) fn tcp_outbound_unsupported() -> TcpOutboundFailure {
    TcpOutboundFailure {
        stage: "no_tcp_outbound",
        error: unsupported_io("this adapter does not provide a TCP outbound"),
        upstream_endpoint: None,
    }
}

pub(in crate::protocol_registry) fn relay_hop_unsupported() -> EngineError {
    unsupported_io("this adapter does not support relay hop")
}

pub(in crate::protocol_registry) fn udp_outbound_unsupported() -> FlowFailure {
    udp_flow_unsupported(
        "no_udp_outbound",
        "this adapter does not provide a UDP outbound",
    )
}

pub(in crate::protocol_registry) fn udp_two_stream_relay_unsupported() -> FlowFailure {
    udp_flow_unsupported(
        "no_two_stream_relay",
        "this adapter does not support two-stream UDP relay",
    )
}

pub(in crate::protocol_registry) fn udp_relay_final_hop_unsupported() -> FlowFailure {
    udp_flow_unsupported(
        "no_udp_relay_final_hop",
        "this adapter does not support UDP relay final hop",
    )
}

pub(in crate::protocol_registry) fn packet_path_carrier_unsupported() -> EngineError {
    unsupported_io("this adapter does not provide a UDP packet-path carrier")
}

pub(crate) fn unreachable_leaf(
    adapter: &'static str,
    _leaf: &ResolvedLeafOutbound<'_>,
) -> TcpOutboundFailure {
    TcpOutboundFailure {
        stage: "outbound_leaf_mismatch",
        error: EngineError::Io(std::io::Error::other(format!(
            "{adapter} adapter received a non-matching outbound leaf"
        ))),
        upstream_endpoint: None,
    }
}

pub(crate) fn unreachable_udp_leaf(
    adapter: &'static str,
    _leaf: &ResolvedLeafOutbound<'_>,
) -> FlowFailure {
    FlowFailure {
        stage: "udp_leaf_mismatch",
        error: EngineError::Io(std::io::Error::other(format!(
            "{adapter} adapter received a non-matching UDP leaf"
        ))),
        upstream: None,
    }
}

fn udp_flow_unsupported(stage: &'static str, message: &'static str) -> FlowFailure {
    FlowFailure {
        stage,
        error: unsupported_io(message),
        upstream: None,
    }
}
