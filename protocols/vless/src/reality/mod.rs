// Portions of this module are adapted from shoes:
// Copyright (c) 2021-2023 Alex Lau <github@alau.ca>, MIT License.

#![allow(dead_code)]

pub(crate) mod buf_reader;
pub(crate) mod common;
pub(crate) mod reality_aead;
pub(crate) mod reality_auth;
pub(crate) mod reality_cipher_suite;
pub(crate) mod reality_client_connection;
pub(crate) mod reality_client_verify;
pub(crate) mod reality_io_state;
pub(crate) mod reality_reader_writer;
pub(crate) mod reality_records;
pub(crate) mod reality_server_connection;
pub(crate) mod reality_tls13_keys;
pub(crate) mod reality_tls13_messages;
pub(crate) mod reality_util;
pub(crate) mod slide_buffer;

pub use stream::{
    generate_reality_key_pair, upgrade_reality_client, upgrade_reality_server,
    upgrade_reality_server_from_config, RealityClientOptions, RealityServerOptions,
    RealityTlsStream,
};

mod stream;
