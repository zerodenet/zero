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

pub use inbound::{TrojanAccept, TrojanInbound, TrojanInboundUdpCodec};
pub use metadata::TrojanProtocol;
pub use outbound::{
    build_udp_request, establish_udp_packet_tunnel, read_inbound_udp_packet, read_udp_flow_packet,
    udp_flow_packet, write_udp_flow_packet, write_udp_response, TrojanOutbound,
    TrojanTcpTunnelTarget, TrojanUdpFlowIo, TrojanUdpFlowKey, TrojanUdpFlowResume,
    TrojanUdpFlowStore, TrojanUdpLeafKey, TrojanUdpPacket, TrojanUdpPacketTunnelTarget,
    TrojanUdpPeerConfig, TrojanUdpTlsProfile,
};
#[cfg(feature = "tokio")]
pub use outbound::{
    establish_udp_flow_with_resume, spawn_udp_flow, TrojanUdpFlowHandle,
    TrojanUdpFlowResponseReceiver, TrojanUdpFlowSession,
};
pub use shared::{
    build_udp_packet, read_password, read_request, read_udp_packet, write_password, write_request,
    write_udp_packet, ATYP_DOMAIN, ATYP_IPV4, ATYP_IPV6, CMD_TCP, CMD_UDP, CRLF, PASSWORD_HASH_LEN,
};

#[cfg(feature = "crypto")]
pub use shared::hex;
