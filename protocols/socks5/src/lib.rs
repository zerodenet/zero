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
    Socks5UdpFlowResume, Socks5UdpRelayTarget,
};
pub use shared::{
    build_udp_packet, decode_udp_associate_request, decode_udp_associate_response,
    encode_udp_associate_response, encode_udp_associate_response_to_client, parse_udp_packet,
    Socks5Reply, Socks5UdpFlowConfig, Socks5UdpPacket,
};
pub use udp::{
    establish_udp_relay_with_control, Socks5InboundUdpCodec, Socks5OwnedUdpAssociationConfig,
    Socks5UdpAssociation, Socks5UdpAssociationConfig, Socks5UdpAssociationTarget, Socks5UdpRelay,
    Socks5UdpRelayEndpoint, Socks5UdpRelayError, Socks5UdpRelayTargetAddress,
};
