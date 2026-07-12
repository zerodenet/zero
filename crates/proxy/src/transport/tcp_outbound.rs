//! TCP outbound data types used by both transport and runtime layers.
//!
//! The TCP pipe orchestration lives in `crate::runtime::tcp_dispatch`.
//! This module keeps only the neutral facade over prepared transport-bridge
//! orchestration shared by transport-backed protocol adapters.

mod connect;
mod error;
mod model;
mod relay;
mod result;

pub(crate) use connect::connect_protocol_transport_bridge_tcp;
pub(crate) use model::{EstablishedTcpOutbound, TcpOutboundFailure, TcpRouteResult};
pub(crate) use relay::apply_protocol_transport_bridge_relay_hop;
pub(crate) use result::extract_tcp_stream;
