#![cfg_attr(not(feature = "tokio"), no_std)]
#![allow(async_fn_in_trait)]

extern crate alloc;

#[cfg(feature = "reality")]
pub mod deferred_response;
#[cfg(feature = "reality")]
pub mod flow;
pub mod inbound;
pub mod metadata;
pub mod mux;
#[cfg(feature = "reality")]
pub mod mux_crypto;
#[cfg(feature = "reality")]
pub mod mux_pool;
pub mod outbound;
#[cfg(feature = "reality")]
pub mod reality;
mod shared;
pub mod udp;

pub use shared::{format_uuid, parse_uuid, VLESS_VERSION};
