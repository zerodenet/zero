//! Generic UDP flow helpers and session state.
//!
//! Protocol-specific SOCKS5 UDP ASSOCIATE handling lives under
//! `inbound::socks5::udp_associate`.

pub(crate) mod helpers;
pub(crate) mod managed;
pub(crate) mod outbound;
pub(crate) mod packet_path;
pub(crate) mod packet_path_chain;
pub(crate) mod protocol_state;
pub(crate) mod sessions;
