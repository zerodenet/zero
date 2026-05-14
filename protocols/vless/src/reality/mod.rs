// Portions of this module are adapted from shoes:
// Copyright (c) 2021-2023 Alex Lau <github@alau.ca>, MIT License.

#![allow(dead_code)]

pub mod buf_reader;
pub mod common;
pub mod reality_aead;
pub mod reality_auth;
pub mod reality_cipher_suite;
pub mod reality_client_connection;
pub mod reality_client_verify;
pub mod reality_io_state;
pub mod reality_reader_writer;
pub mod reality_records;
pub mod reality_server_connection;
pub mod reality_tls13_keys;
pub mod reality_tls13_messages;
pub mod reality_util;
pub mod slide_buffer;

pub use stream::{
    generate_reality_key_pair, upgrade_reality_client, upgrade_reality_server,
    RealityClientOptions, RealityServerOptions, RealityTlsStream,
};

mod stream;
