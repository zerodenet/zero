//! Generic UDP flow helpers and session state.
//!
//! Protocol-specific inbound UDP association/session handling lives under the
//! owning adapter listener modules.

pub(crate) mod helpers;
pub(crate) mod managed;
pub(crate) mod outbound;
pub(crate) mod packet_path;
pub(crate) mod packet_path_chain;
pub(crate) mod registered;
pub(crate) mod response;
pub(crate) mod sessions;
pub(crate) mod state;
