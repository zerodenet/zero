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
pub use outbound::{TrojanOutbound, TrojanTcpTunnelTarget};
pub use shared::{
    read_password, read_request, write_password, write_request, ATYP_DOMAIN, ATYP_IPV4, ATYP_IPV6,
    CMD_TCP, CMD_UDP, CRLF, PASSWORD_HASH_LEN,
};

#[cfg(feature = "crypto")]
pub use shared::hex;
