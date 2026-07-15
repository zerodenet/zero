//! Hysteria2 inbound profile preparation.

use ::hysteria2::transport::Hysteria2AuthenticatedInboundProfile;

use crate::runtime::inbound_operation::AuthenticatedQuicInboundListenerOperation;

pub(super) fn prepare(
    profile: Hysteria2AuthenticatedInboundProfile,
) -> Box<dyn crate::runtime::inbound_operation::PreparedInboundListenerOperation> {
    Box::new(AuthenticatedQuicInboundListenerOperation {
        protocol_name: "hysteria2",
        profile,
    })
}
