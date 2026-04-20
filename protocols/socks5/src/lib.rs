#![no_std]
#![allow(async_fn_in_trait)]

extern crate alloc;

mod inbound;
mod outbound;
mod shared;

pub use inbound::Socks5Inbound;
pub use outbound::Socks5Outbound;
pub use shared::Socks5Reply;
