#![no_std]
#![allow(async_fn_in_trait)]

extern crate alloc;

mod inbound;
mod outbound;
mod shared;
mod udp;

pub use inbound::{
    NoSocks5PasswordAuth, Socks5Inbound, Socks5PasswordAuth, Socks5Request,
    Socks5UdpAssociateRequest,
};
pub use outbound::{Socks5Outbound, Socks5OutboundAuth};
pub use shared::{build_udp_packet, parse_udp_packet, Socks5Reply, Socks5UdpPacket};
pub use udp::{Socks5UdpRelay, Socks5UdpRelayEndpoint, Socks5UdpRelayError};
