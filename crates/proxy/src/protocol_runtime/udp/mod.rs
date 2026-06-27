//! UDP capability handler glue.
//!
//! Generic UDP dispatch owns flow lifecycle and adapter dispatch. This module
//! keeps the remaining handler registry glue while protocol-private flow
//! resumes and codecs live behind adapter-registered capability objects.

mod flows;
mod start;
mod state;

pub(crate) use crate::runtime::udp_dispatch::FlowFailure;
pub(crate) use flows::{ManagedUdpFlowKind, ManagedUdpFlowRequest};
pub(crate) use state::{
    CachedUdpHandlers, ManagedCachedFlowSender, ManagedDatagramFlowHandler, ManagedExistingSend,
    ManagedRelaySend, ManagedStreamFlowHandler, ManagedUdpHandlers, ProtocolUdpHandlers,
    ProtocolUdpState, UpstreamAssociationHandler, UpstreamUdpHandlers,
};
