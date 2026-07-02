#![no_std]
#![allow(async_fn_in_trait)]

extern crate alloc;

mod inbound;
mod metadata;
mod outbound;
mod shared;
pub mod udp;

pub use inbound::{
    is_socks5_greeting_byte, ConfiguredSocks5PasswordAuth, ConfiguredSocks5User,
    IntoSocks5AuthUserConfig, NoSocks5PasswordAuth, Socks5Inbound, Socks5InboundTcpAcceptor,
    Socks5PasswordAuth, Socks5Request,
};
pub use metadata::Socks5Protocol;
pub use outbound::{
    Socks5Outbound, Socks5OutboundAuth, Socks5OwnedOutboundAuth, Socks5TcpConnectSpec,
    Socks5TcpOutboundProfile,
};
pub use shared::Socks5Reply;
