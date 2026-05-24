#![cfg_attr(not(feature = "crypto"), no_std)]
#![allow(async_fn_in_trait)]
#![allow(clippy::wrong_self_convention)]

extern crate alloc;

mod inbound;
mod outbound;
pub mod shared;

#[cfg(feature = "crypto")]
pub use inbound::ShadowsocksAccept;
pub use inbound::ShadowsocksInbound;
pub use outbound::ShadowsocksOutbound;
#[cfg(feature = "blake3")]
pub use shared::derive_key_blake3;
#[cfg(feature = "crypto")]
pub use shared::{aead_decrypt, aead_decrypt_udp, aead_encrypt, aead_encrypt_udp, derive_key};
pub use shared::{
    build_target_data, decode_address, encode_address, parse_target_data, read_exact, CipherKind,
    ADDR_TYPE_DOMAIN, ADDR_TYPE_IPV4, ADDR_TYPE_IPV6,
};
