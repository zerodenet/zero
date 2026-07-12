use super::super::datagram::ManagedDatagramState;
use super::super::flow::ManagedUdpFlowResume;
use super::super::model::{ManagedDatagramFlowHandler, ManagedStreamFlowHandler};
use super::super::stream::ManagedStreamState;
use crate::runtime::udp_flow::outbound::ManagedUdpFlowRef;
use std::collections::HashMap;

pub(crate) struct ManagedUdpHandlers {
    pub(crate) datagram: Vec<Box<dyn ManagedDatagramFlowHandler>>,
    pub(crate) stream: Vec<Box<dyn ManagedStreamFlowHandler>>,
}

pub(crate) struct ManagedUdpState {
    pub(super) datagram: ManagedDatagramState,
    pub(super) stream: ManagedStreamState,
    pub(super) flows: HashMap<ManagedUdpFlowRef, ManagedUdpFlowResume>,
    pub(super) next_flow_id: u64,
}
