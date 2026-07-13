//! Hysteria2 inbound profile preparation.

use zero_engine::EngineError;

use crate::runtime::inbound_operation::AuthenticatedQuicInboundListenerOperation;

impl crate::adapters::hysteria2::Hysteria2Adapter {
    pub(super) fn prepare_inbound_listener_impl(
        &self,
        inbound: zero_config::InboundConfig,
    ) -> Result<
        Box<dyn crate::runtime::inbound_operation::PreparedInboundListenerOperation>,
        EngineError,
    > {
        let profile =
            zero_transport::hysteria2_quic::inbound_profile_from_protocol(&inbound.protocol)?;
        Ok(Box::new(AuthenticatedQuicInboundListenerOperation {
            inbound_tag: inbound.tag,
            protocol_name: "hysteria2",
            profile,
        }))
    }
}
