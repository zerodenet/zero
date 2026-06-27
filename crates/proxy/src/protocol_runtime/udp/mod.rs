//! UDP capability handler glue.
//!
//! Generic UDP dispatch owns flow lifecycle and adapter dispatch. This module
//! keeps the remaining handler registry glue while protocol-private flow
//! resumes and codecs live behind adapter-registered capability objects.

mod start;
mod state;

pub(crate) use crate::runtime::udp_dispatch::FlowFailure;
pub(crate) use state::{
    CachedUdpHandlers, ProtocolUdpHandlers, ProtocolUdpState, UpstreamAssociationHandler,
    UpstreamUdpHandlers,
};
