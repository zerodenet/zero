#![no_std]
#![allow(async_fn_in_trait)]

extern crate alloc;
#[cfg(feature = "runtime")]
extern crate std;

mod inbound;
mod metadata;
mod outbound;
mod shared;
#[cfg(feature = "runtime")]
pub mod transport;
pub mod udp;

pub use inbound::{
    is_socks5_greeting_byte, ConfiguredSocks5PasswordAuth, ConfiguredSocks5User,
    IntoSocks5AuthUserConfig, NoSocks5PasswordAuth, Socks5Inbound, Socks5InboundTcpAcceptor,
    Socks5PasswordAuth, Socks5Request,
};
pub use metadata::Socks5Protocol;
pub use outbound::{
    validate_credential_part, Socks5Outbound, Socks5OutboundAuth, Socks5OwnedOutboundAuth,
    Socks5TcpConnectSpec, Socks5TcpOutboundProfile,
};
pub use shared::Socks5Reply;
