//! TCP outbound data types used by both transport and runtime layers.
//!
//! The TCP pipe orchestration lives in `crate::runtime::tcp_dispatch`.
//! This module keeps only the neutral facade over prepared transport-bridge
//! orchestration shared by transport-backed protocol adapters.

#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
mod connect;
mod error;
mod model;
#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
mod relay;
mod result;

#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
pub(crate) use connect::connect_protocol_transport_bridge_tcp;
pub(crate) use error::is_block_error;
pub(crate) use model::{EstablishedTcpOutbound, TcpOutboundFailure, TcpRouteResult};
#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
pub(crate) use relay::apply_protocol_transport_bridge_relay_hop;
pub(crate) use result::extract_tcp_stream;
