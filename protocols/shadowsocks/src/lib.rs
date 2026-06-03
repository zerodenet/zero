#![cfg_attr(not(feature = "crypto"), no_std)]
#![allow(async_fn_in_trait)]

extern crate alloc;

mod inbound;
mod outbound;
pub mod shared;

#[cfg(feature = "crypto")]
pub use inbound::ShadowsocksAccept;
pub use inbound::ShadowsocksInbound;
pub use outbound::ShadowsocksOutbound;
#[cfg(feature = "crypto")]
pub use outbound::ShadowsocksOutboundSession;
#[cfg(feature = "blake3")]
pub use shared::derive_key_blake3;
#[cfg(feature = "crypto")]
pub use shared::{
    aead_decrypt, aead_decrypt_udp, aead_encrypt, aead_encrypt_udp, decrypt_tcp_chunk_length,
    decrypt_tcp_chunk_payload, derive_key, encrypt_tcp_chunk, read_tcp_chunk, TCP_CHUNK_SIZE_LEN,
};
pub use shared::{
    build_target_data, decode_address, encode_address, parse_target_data, read_exact, CipherKind,
    ADDR_TYPE_DOMAIN, ADDR_TYPE_IPV4, ADDR_TYPE_IPV6,
};
