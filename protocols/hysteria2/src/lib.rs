#![cfg_attr(not(feature = "crypto"), no_std)]
#![allow(async_fn_in_trait)]

extern crate alloc;

mod inbound;
mod metadata;
mod outbound;
pub mod shared;
pub mod udp;

#[cfg(feature = "crypto")]
pub use inbound::{inbound_profile_from_config_password, Hysteria2InboundProfile};
pub use inbound::{Hysteria2Inbound, Hysteria2User, Hysteria2UserStore};
pub use metadata::Hysteria2Protocol;
pub use outbound::Hysteria2Outbound;
#[cfg(feature = "crypto")]
pub use outbound::{outbound_profile_from_config_password, Hysteria2OutboundProfile};
#[cfg(feature = "tokio")]
pub use udp::Hysteria2InboundUdpResponder;
