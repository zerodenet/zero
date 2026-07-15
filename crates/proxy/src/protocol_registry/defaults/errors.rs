use zero_engine::EngineError;

#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
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

#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
pub(in crate::protocol_registry) fn udp_two_stream_relay_unsupported() -> FlowFailure {
    udp_flow_unsupported(
        "no_two_stream_relay",
        "this adapter does not support two-stream UDP relay",
    )
}

#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
pub(in crate::protocol_registry) fn udp_relay_final_hop_unsupported() -> FlowFailure {
    udp_flow_unsupported(
        "no_udp_relay_final_hop",
        "this adapter does not support UDP relay final hop",
    )
}

#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
fn udp_flow_unsupported(stage: &'static str, message: &'static str) -> FlowFailure {
    FlowFailure {
        stage,
        error: unsupported_io(message),
        upstream: None,
    }
}
