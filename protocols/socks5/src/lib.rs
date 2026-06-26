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
    Socks5Outbound, Socks5OutboundAuth, Socks5TcpTunnelTarget, Socks5UdpFlowResume,
    Socks5UdpRelayTarget,
};
pub use shared::{
    build_udp_packet, decode_udp_associate_request, decode_udp_associate_response,
    encode_udp_associate_response, parse_udp_packet, udp_cache_key, Socks5Reply, Socks5UdpPacket,
    Socks5UdpPacketPathConfig,
};
pub use udp::{
    establish_udp_relay_with_control, Socks5UdpRelay, Socks5UdpRelayEndpoint, Socks5UdpRelayError,
    Socks5UdpRelayTargetAddress,
};
