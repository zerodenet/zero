#![cfg_attr(not(feature = "reality"), no_std)]
#![allow(async_fn_in_trait)]

extern crate alloc;

mod inbound;
mod outbound;
#[cfg(feature = "reality")]
mod reality;
mod shared;

pub use inbound::{VlessInbound, VlessUser, VlessUserStore};
pub use outbound::VlessOutbound;
#[cfg(feature = "reality")]
pub use reality::{upgrade_reality_client, RealityClientOptions, RealityTlsStream};
pub use shared::{format_uuid, parse_uuid, VLESS_VERSION};
