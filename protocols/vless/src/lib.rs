#![no_std]
#![allow(async_fn_in_trait)]

extern crate alloc;

mod inbound;
mod outbound;
mod shared;

pub use inbound::{VlessInbound, VlessUser, VlessUserStore};
pub use outbound::VlessOutbound;
pub use shared::{format_uuid, parse_uuid, VLESS_VERSION};
