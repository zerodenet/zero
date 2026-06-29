// Mieru protocol — lib.rs
//
// Implements the mieru proxy protocol (https://github.com/enfein/mieru).
// XChaCha20-Poly1305 AEAD, time-based key derivation, session lifecycle,
// TCP + UDP transport with random padding anti-detection.

#![cfg_attr(not(feature = "crypto"), no_std)]
#![allow(async_fn_in_trait)]

extern crate alloc;

pub mod metadata;
pub mod protocol;
#[cfg(feature = "crypto")]
pub mod segment;
#[cfg(feature = "crypto")]
pub mod session;
pub mod udp;

#[cfg(feature = "crypto")]
pub mod crypto;

#[cfg(feature = "crypto")]
mod inbound;
#[cfg(feature = "crypto")]
mod outbound;

#[cfg(feature = "crypto")]
pub use inbound::{
    classify_inbound_session, IntoMieruInboundUserConfig, MieruAccept, MieruInbound,
    MieruInboundProfile, MieruInboundSessionKind, MieruInboundStream,
};
#[cfg(feature = "crypto")]
pub use outbound::{
    establish_tcp_tunnel, MieruOutbound, MieruTcpOutboundProfile, MieruTcpStream, MieruTcpTarget,
    MieruTcpTunnelTarget,
};
pub use protocol::MieruProtocol;
