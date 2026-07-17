use super::ProtocolRegistry;
use zero_api::{
    CapabilityState as ApiCapabilityState, ProtocolCapability,
    ProtocolNetworkCapability as ApiProtocolNetworkCapability,
};
use zero_traits::{
    ProtocolCapabilityDescriptor, ProtocolCapabilityLevel, ProtocolCapabilityState,
    ProtocolNetworkCapability,
};

impl ProtocolRegistry {
    pub(crate) fn compiled_feature_names(&self) -> Vec<&'static str> {
        let mut features = self
            .entries
            .iter()
            .map(|entry| entry.support.feature_name())
            .filter(|feature| *feature != "core")
            .collect::<Vec<_>>();
        features.sort_unstable();
        features.dedup();
        features
    }

    /// Names of all compiled-in inbound protocols.
    pub(crate) fn inbound_names(&self) -> Vec<&'static str> {
        self.entries
            .iter()
            .filter(|entry| entry.support.has_inbound())
            .map(|entry| entry.support.name())
            .collect::<Vec<_>>()
    }

    /// Names of all compiled-in outbound protocols.
    pub(crate) fn outbound_names(&self) -> Vec<&'static str> {
        let mut names: Vec<&'static str> = vec!["direct", "block"];
        names.extend(
            self.entries
                .iter()
                .filter(|entry| entry.support.has_outbound())
                .map(|entry| entry.support.name()),
        );
        names
    }

    pub(crate) fn capabilities(&self) -> Vec<zero_api::ProtocolCapability> {
        let mut descriptors = self
            .entries
            .iter()
            .map(|entry| entry.support.descriptor())
            .collect::<Vec<_>>();

        if !descriptors
            .iter()
            .any(|descriptor| descriptor.protocol == "block")
        {
            descriptors.push(block_descriptor());
        }

        let mut capabilities = descriptors
            .into_iter()
            .map(protocol_capability)
            .collect::<Vec<_>>();
        capabilities.sort_by(|a, b| a.protocol.cmp(&b.protocol));
        capabilities
    }
}

fn block_descriptor() -> ProtocolCapabilityDescriptor {
    ProtocolCapabilityDescriptor {
        protocol: "block",
        feature: "core",
        status: ProtocolCapabilityLevel::Supported,
        compatibility_baseline: "kernel_builtin",
        inbound: ProtocolNetworkCapability::new(
            ProtocolCapabilityState::unsupported(&[]),
            ProtocolCapabilityState::unsupported(&[]),
        ),
        outbound: ProtocolNetworkCapability::new(
            ProtocolCapabilityState::supported(),
            ProtocolCapabilityState::supported(),
        ),
        transports: &["tcp", "udp"],
        mux: ProtocolCapabilityState::not_applicable(),
        limitations: &[],
    }
}

fn protocol_capability(descriptor: ProtocolCapabilityDescriptor) -> ProtocolCapability {
    ProtocolCapability {
        protocol: descriptor.protocol.to_owned(),
        feature: descriptor.feature.to_owned(),
        compiled: true,
        status: descriptor.status.as_str().to_owned(),
        compatibility_baseline: descriptor.compatibility_baseline.to_owned(),
        inbound: api_network(descriptor.inbound),
        outbound: api_network(descriptor.outbound),
        transports: descriptor
            .transports
            .iter()
            .map(|value| (*value).to_owned())
            .collect(),
        mux: api_state(descriptor.mux),
        limitations: descriptor
            .limitations
            .iter()
            .map(|value| (*value).to_owned())
            .collect(),
    }
}

fn api_network(capability: ProtocolNetworkCapability) -> ApiProtocolNetworkCapability {
    ApiProtocolNetworkCapability {
        tcp: api_state(capability.tcp),
        udp: api_state(capability.udp),
    }
}

fn api_state(state: ProtocolCapabilityState) -> ApiCapabilityState {
    ApiCapabilityState {
        supported: state.supported,
        level: state.level.as_str().to_owned(),
        notes: state
            .notes
            .iter()
            .map(|value| (*value).to_owned())
            .collect(),
    }
}
