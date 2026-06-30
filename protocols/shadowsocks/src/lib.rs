#![cfg_attr(not(feature = "crypto"), no_std)]
#![allow(async_fn_in_trait)]

extern crate alloc;

mod inbound;
mod metadata;
mod outbound;
pub mod shared;
#[cfg(feature = "crypto")]
mod stream;
pub mod udp;

pub use inbound::ShadowsocksInbound;
#[cfg(feature = "crypto")]
pub use inbound::{
    ShadowsocksAccept, ShadowsocksInboundProfile, ShadowsocksInboundTcpState,
    ShadowsocksInboundUdpResponder,
};
pub use metadata::ShadowsocksProtocol;
pub use outbound::ShadowsocksOutbound;
#[cfg(feature = "crypto")]
pub use outbound::{
    tcp_connect_config_from_config, ShadowsocksOutboundSession, ShadowsocksTcpConnectConfig,
};
#[cfg(feature = "crypto")]
pub use shared::{
    aead_decrypt, aead_encrypt, decrypt_tcp_chunk_length, decrypt_tcp_chunk_payload,
    derive_download_key, derive_key, derive_session_key, encrypt_tcp_chunk, read_tcp_chunk,
    write_tcp_chunk, TCP_CHUNK_SIZE_LEN,
};
pub use shared::{
    build_2022_request_fixed_header, build_2022_request_var_header,
    build_2022_response_fixed_header, parse_2022_request_fixed_header,
    parse_2022_request_var_header, parse_2022_response_fixed_header,
    ss_2022_response_header_plain_len, SS_2022_HEADER_TYPE_CLIENT_STREAM,
    SS_2022_HEADER_TYPE_SERVER_STREAM, SS_2022_MAX_PADDING_LENGTH,
    SS_2022_REQUEST_FIXED_HEADER_LEN, SS_2022_TIMESTAMP_WINDOW_SECS,
};
pub use shared::{
    build_target_data, decode_address, encode_address, parse_target_data, read_exact, CipherKind,
    ADDR_TYPE_DOMAIN, ADDR_TYPE_IPV4, ADDR_TYPE_IPV6,
};
#[cfg(feature = "blake3")]
pub use shared::{decode_blake3_master_key, derive_key_blake3};
#[cfg(feature = "crypto")]
pub use shared::{
    decrypt_tcp_2022_single_chunk, encrypt_tcp_2022_single_chunk, max_tcp_payload_len,
};
#[cfg(all(feature = "crypto", feature = "blake3"))]
pub use shared::{
    now_unix_seconds, random_2022_padding, validate_2022_timestamp, ReplaySaltPool, ReplayWindow,
};
#[cfg(feature = "crypto")]
pub use stream::ShadowsocksAeadStream;
