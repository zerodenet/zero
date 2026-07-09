#![cfg_attr(not(feature = "crypto"), no_std)]
#![allow(async_fn_in_trait)]

extern crate alloc;

pub mod inbound;
mod metadata;
mod outbound;
pub mod shared;
pub mod udp;

pub use metadata::Hysteria2Protocol;
pub use outbound::Hysteria2Outbound;
#[cfg(feature = "crypto")]
pub use outbound::{outbound_profile_from_config_password, Hysteria2OutboundProfile};
