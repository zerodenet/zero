//! Proxy transport facade.
//!
//! This module re-exports neutral stream and QUIC contracts from
//! `zero-transport` and owns proxy-specific direct connect, TCP outbound
//! normalization, relay-chain handoff, metering, and rate-limited relay glue.
//! Concrete carrier implementations remain in the `zero-transport` crate.

mod direct;
mod tcp_outbound;
mod tcp_relay;

pub(crate) use direct::DirectConnector;
pub(crate) use tcp_outbound::{
    extract_tcp_stream, is_block_error, EstablishedTcpOutbound, TcpOutboundFailure, TcpRouteResult,
};
#[cfg(feature = "vless")]
pub(crate) use tcp_relay::relay_bidirectional_metered;
pub(crate) use tcp_relay::relay_bidirectional_metered_throttled;
#[cfg(any(feature = "socks5", feature = "vless"))]
pub(crate) use zero_transport::ClientStream;
#[cfg(any(
    feature = "socks5",
    feature = "http",
    feature = "mixed",
    feature = "vless",
    feature = "shadowsocks",
    feature = "mieru"
))]
pub(crate) use zero_transport::MeteredStream;
#[cfg(feature = "mixed")]
pub(crate) use zero_transport::PrefixedSocket;
#[cfg(feature = "vless")]
pub(crate) use zero_transport::RecordingStream;
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
pub(crate) use zero_transport::StreamTraffic;
pub(crate) use zero_transport::{RelayCarrier, TcpRelayStream};

// Re-export transport implementations from zero-transport.
// Only items used directly by proxy code are listed.
#[cfg(any(feature = "hysteria2", feature = "vless"))]
pub(crate) use zero_transport::quic::QuicInbound;
#[cfg(feature = "vless")]
pub(crate) use zero_transport::quic::QuicStream;
