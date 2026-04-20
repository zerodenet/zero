#![no_std]
#![allow(async_fn_in_trait)]

extern crate alloc;

mod inbound;
mod parse;

pub use inbound::{HttpConnectInbound, HttpConnectResponse};
