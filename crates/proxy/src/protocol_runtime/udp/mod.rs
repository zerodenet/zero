//! Protocol-specific UDP runtime managers.
//!
//! Generic UDP dispatch owns flow lifecycle and adapter dispatch. Concrete
//! protocol managers live here and are called through request structs.

mod flow_snapshot;
mod flows;
mod start;
mod state;

pub(crate) use crate::runtime::udp_dispatch::FlowFailure;
pub(crate) use flow_snapshot::{ProtocolUdpFlowResume, ProtocolUdpFlowSnapshot};
pub(crate) use flows::{ManagedUdpFlowKind, ManagedUdpFlowRequest};
pub(crate) use state::{CachedUdpHandlers, ProtocolUdpHandlers, ProtocolUdpState};
pub(crate) use state::{
    ManagedCachedFlowSender, ManagedDatagramFlowHandler, ManagedExistingSend, ManagedRelaySend,
    ManagedStreamFlowHandler, ManagedUdpHandlers,
};
