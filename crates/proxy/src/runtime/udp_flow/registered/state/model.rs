use crate::runtime::udp_flow::managed::{ManagedUdpHandlers, ManagedUdpState};

use super::super::upstream::UpstreamAssociationState;

pub(crate) struct RegisteredUpstreamAssociationView<'a> {
    pub(crate) outbound_tag: &'a str,
}

pub(crate) struct ClosedRegisteredUpstreamAssociation {
    pub(crate) outbound_tag: String,
    pub(crate) server: String,
    pub(crate) port: u16,
}

pub(crate) struct RegisteredUdpState {
    pub(in crate::runtime::udp_flow::registered) managed: ManagedUdpState,
    pub(in crate::runtime::udp_flow::registered) upstream: UpstreamAssociationState,
}

pub(crate) struct RegisteredUdpHandlers {
    pub(crate) managed: ManagedUdpHandlers,
    pub(crate) upstream: super::super::upstream::UpstreamUdpHandlers,
}
