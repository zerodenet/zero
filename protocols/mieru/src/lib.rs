// Mieru protocol — lib.rs
//
// Implements the mieru proxy protocol (https://github.com/enfein/mieru).
// XChaCha20-Poly1305 AEAD, time-based key derivation, session lifecycle,
// TCP + UDP transport with random padding anti-detection.

#![cfg_attr(not(feature = "crypto"), no_std)]
#![allow(async_fn_in_trait)]

extern crate alloc;

pub mod metadata;
pub mod segment;
pub mod session;
pub mod udp;

#[cfg(feature = "crypto")]
pub mod crypto;

mod inbound;
mod outbound;

#[cfg(feature = "crypto")]
pub use crypto::{
    derive_key, try_derive_keys, MieruCipher, NonceConfig, NoncePattern, USER_HINT_LEN,
};

pub use inbound::MieruInbound;
pub use metadata::{
    DataMetadata, SessionMetadata, ACK_CLIENT_TO_SERVER, ACK_SERVER_TO_CLIENT,
    CLOSE_SESSION_REQUEST, CLOSE_SESSION_RESPONSE, DATA_CLIENT_TO_SERVER, DATA_SERVER_TO_CLIENT,
    METADATA_LEN, OPEN_SESSION_REQUEST, OPEN_SESSION_RESPONSE,
};
pub use outbound::MieruOutbound;
pub use segment::{
    build_data_segment, build_session_segment, parse_segment, Segment, MAX_FRAGMENT,
};
pub use session::MieruSession;
pub use udp::{unwrap_udp_associate, wrap_udp_associate};
