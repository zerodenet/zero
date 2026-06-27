use super::ProtocolRegistry;
use crate::protocol_capability::{protocol_capability, protocol_descriptor};
use crate::protocol_registry::ProtocolSupportCapability;

impl ProtocolRegistry {
    /// Names of all compiled-in inbound protocols.
    pub(crate) fn inbound_names(&self) -> Vec<&'static str> {
        self.adapters
            .iter()
            .filter(|a| ProtocolSupportCapability::has_inbound(a.as_ref()))
            .map(|a| ProtocolSupportCapability::name(a.as_ref()))
            .collect::<Vec<_>>()
    }

    /// Names of all compiled-in outbound protocols.
    pub(crate) fn outbound_names(&self) -> Vec<&'static str> {
        let mut names: Vec<&'static str> = vec!["direct", "block"];
        names.extend(
            self.adapters
                .iter()
                .filter(|a| ProtocolSupportCapability::has_outbound(a.as_ref()))
                .map(|a| ProtocolSupportCapability::name(a.as_ref())),
        );
        names
    }

    pub(crate) fn capabilities(&self) -> Vec<zero_api::ProtocolCapability> {
        let mut descriptors = self
            .adapters
            .iter()
            .map(|adapter| adapter.descriptor())
            .collect::<Vec<_>>();

        if !descriptors
            .iter()
            .any(|descriptor| descriptor.protocol == "block")
        {
            descriptors.push(protocol_descriptor("block", "core"));
        }

        let mut capabilities = descriptors
            .into_iter()
            .map(protocol_capability)
            .collect::<Vec<_>>();
        capabilities.sort_by(|a, b| a.protocol.cmp(&b.protocol));
        capabilities
    }
}
