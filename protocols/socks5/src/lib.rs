#![no_std]
#![allow(async_fn_in_trait)]

extern crate alloc;

mod inbound;
mod metadata;
mod outbound;
mod shared;
mod udp;

pub use inbound::{
    NoSocks5PasswordAuth, Socks5Inbound, Socks5PasswordAuth, Socks5Request,
    Socks5UdpAssociateRequest,
};
pub use metadata::Socks5Protocol;
pub use outbound::{
    Socks5Outbound, Socks5OutboundAuth, Socks5OwnedOutboundAuth, Socks5TcpTunnelTarget,
    Socks5UdpFlowResume, Socks5UdpFlowSpec, Socks5UdpRelayTarget,
};
pub use shared::{
    udp_flow_resume_from_config, udp_packet_path_spec_from_config, Socks5InboundUdpRequest,
    Socks5InboundUdpResponse, Socks5Reply, Socks5UdpFlowConfig, Socks5UdpPacketPathCarrier,
    Socks5UdpPacketPathCarrierBuild, Socks5UdpPacketPathSpec,
};
pub use udp::{
    establish_udp_relay_with_control, Socks5EstablishedUdpAssociation, Socks5InboundUdpCodec,
    Socks5InboundUdpSession, Socks5OwnedUdpAssociationConfig, Socks5UdpAssociation,
    Socks5UdpAssociationConfig, Socks5UdpAssociationTarget, Socks5UdpRelay, Socks5UdpRelayEndpoint,
    Socks5UdpRelayError, Socks5UdpRelayTargetAddress,
};
