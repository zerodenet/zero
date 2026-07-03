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

pub use inbound::{
    classify_inbound_session, inbound_profile_from_config_password, TrojanAccept, TrojanInbound,
    TrojanInboundAcceptedSession, TrojanInboundAcceptedSessionDispatcher, TrojanInboundProfile,
    TrojanInboundSessionKind, TrojanInboundUdpRelay,
};
pub use metadata::TrojanProtocol;
pub use outbound::{
    tcp_connect_config_from_config, tcp_outbound_profile_from_config_password,
    tcp_tls_profile_from_config, TrojanOutbound, TrojanTcpConnectConfig, TrojanTcpOutboundProfile,
    TrojanTcpTlsProfile, TrojanTcpTunnelTarget,
};
