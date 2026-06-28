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

pub use inbound::{
    TrojanAccept, TrojanInbound, TrojanInboundUdpCodec, TrojanInboundUdpDispatchParts,
    TrojanInboundUdpRequest, TrojanInboundUdpSession,
};
pub use metadata::TrojanProtocol;
pub use outbound::{
    build_udp_request, connector_flow_from_resume, establish_udp_packet_tunnel,
    udp_flow_resume_from_config, TrojanOutbound, TrojanTcpTunnelTarget, TrojanUdpConnectorFlow,
    TrojanUdpFlowConfig, TrojanUdpFlowIo, TrojanUdpFlowResume, TrojanUdpPacket,
    TrojanUdpPacketTunnelTarget, TrojanUdpTlsProfile, TrojanUdpTlsProfileSpec,
};
#[cfg(feature = "tokio")]
pub use outbound::{
    establish_udp_flow_with_resume, spawn_udp_flow, TrojanUdpFlowConnection, TrojanUdpFlowHandle,
    TrojanUdpFlowResponseReceiver, TrojanUdpFlowSession, TrojanUdpFlowSessions,
};
pub use shared::{
    read_password, read_request, write_password, write_request, ATYP_DOMAIN, ATYP_IPV4, ATYP_IPV6,
    CMD_TCP, CMD_UDP, CRLF, PASSWORD_HASH_LEN,
};

#[cfg(feature = "crypto")]
pub use shared::hex;
