use std::sync::Arc;

use zero_engine::{EngineError, ResolvedLeafOutbound};

use super::ProtocolRegistry;
use crate::protocol_registry::{
    OutboundLeafRuntime, RegisteredProtocolCapability, TcpOutboundCapability,
};
use crate::runtime::orchestration::TcpPathCategory;

impl ProtocolRegistry {
    /// Find the adapter that owns this resolved outbound leaf, if any.
    ///
    /// Single dispatch point: the TCP/UDP runtime resolves a
    /// [`ResolvedLeafOutbound`] to its adapter here instead of matching on
    /// the protocol enum. Each adapter claims exactly its own variant via
    /// [`TcpOutboundCapability::claims_outbound_leaf`].
    pub(crate) fn find_outbound_leaf(
        &self,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<Arc<dyn RegisteredProtocolCapability>, EngineError> {
        for adapter in &self.adapters {
            if TcpOutboundCapability::claims_outbound_leaf(adapter.as_ref(), leaf) {
                return Ok(adapter.clone());
            }
        }
        Err(EngineError::Io(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "no compiled adapter handles this outbound leaf",
        )))
    }

    /// Return neutral runtime facts for a resolved outbound leaf.
    ///
    /// Kernel-level `block` is handled here because no adapter owns it.
    /// Direct and proxy protocols are delegated to the adapter that claims the
    /// leaf, so runtime code does not match protocol variants.
    pub(crate) fn outbound_leaf_runtime<'a>(
        &self,
        leaf: &ResolvedLeafOutbound<'a>,
    ) -> Result<OutboundLeafRuntime<'a>, EngineError> {
        if let ResolvedLeafOutbound::Block { tag } = leaf {
            return Ok(OutboundLeafRuntime {
                tcp_path: TcpPathCategory::Block,
                health_tag: None,
                endpoint: None,
                kernel_tag: *tag,
                udp_policy_tag: *tag,
            });
        }

        for adapter in &self.adapters {
            if !TcpOutboundCapability::claims_outbound_leaf(adapter.as_ref(), leaf) {
                continue;
            }
            if let Some(runtime) =
                TcpOutboundCapability::outbound_leaf_runtime(adapter.as_ref(), leaf)
            {
                return Ok(runtime);
            }
            break;
        }

        Err(EngineError::Io(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "no compiled adapter describes this outbound leaf",
        )))
    }
}
