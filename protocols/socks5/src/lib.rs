#![no_std]
#![allow(async_fn_in_trait)]

extern crate alloc;

mod inbound;
mod outbound;
mod shared;

pub use inbound::{Socks5Inbound, Socks5Request, Socks5UdpAssociateRequest};
pub use outbound::Socks5Outbound;
pub use shared::{build_udp_packet, parse_udp_packet, Socks5Reply, Socks5UdpPacket};
