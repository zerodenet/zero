#![cfg_attr(not(feature = "quic"), no_std)]
#![allow(async_fn_in_trait)]

extern crate alloc;

mod inbound;
mod outbound;
pub mod shared;
#[cfg(feature = "quic")]
mod stream;
#[cfg(feature = "quic")]
pub mod udp;

pub use inbound::{Hysteria2Inbound, Hysteria2User, Hysteria2UserStore};
pub use outbound::Hysteria2Outbound;
pub use shared::{
    build_auth_error, build_auth_frame, build_auth_ok, build_connect_error, build_connect_ok,
    build_tcp_connect_header, parse_auth_frame, parse_auth_response, parse_tcp_connect_header,
    ADDR_TYPE_DOMAIN, ADDR_TYPE_IPV4, ADDR_TYPE_IPV6, AUTH_ERR, AUTH_OK,
    HYSTERIA2_VERSION, STREAM_TYPE_TCP, STREAM_TYPE_UDP,
};
#[cfg(feature = "quic")]
pub use stream::Hysteria2Stream;
#[cfg(feature = "quic")]
pub use udp::{build_udp_datagram, parse_udp_datagram, Hysteria2UdpPacket};
