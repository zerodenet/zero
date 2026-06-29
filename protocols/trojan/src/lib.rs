//! Trojan protocol implementation (trojan-go spec).
//!
//! Trojan tunnels TCP/UDP over TLS with password authentication.
//! The upstream server validates the password, reads the target address,
//! and relays traffic.

#![allow(async_fn_in_trait)]

mod inbound;
mod metadata;
mod outbound;
pub mod shared;
pub mod udp;

pub use inbound::{TrojanAccept, TrojanInbound, TrojanInboundProfile};
pub use metadata::TrojanProtocol;
pub use outbound::{
    TrojanOutbound, TrojanTcpOutboundProfile, TrojanTcpTlsProfile, TrojanTcpTunnelTarget,
};
