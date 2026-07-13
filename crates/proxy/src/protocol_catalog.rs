//! Kernel protocol descriptors and control-plane capability projections.

use zero_api::{
    CapabilityState as ApiCapabilityState, ProtocolCapability,
    ProtocolNetworkCapability as ApiProtocolNetworkCapability,
};
use zero_traits::{
    ProtocolCapabilityDescriptor, ProtocolCapabilityLevel, ProtocolCapabilityState,
    ProtocolNetworkCapability,
};

pub(crate) fn protocol_descriptor(
    protocol: &'static str,
    feature: &'static str,
) -> ProtocolCapabilityDescriptor {
    let unsupported = ProtocolCapabilityState::unsupported(&[]);
    let supported = ProtocolCapabilityState::supported();
    let not_applicable = ProtocolCapabilityState::not_applicable();

    // Only kernel actions remain here. Protocol crates own their own descriptors.
    let (status, compatibility_baseline, inbound, outbound, transports, mux, limitations) =
        match protocol {
            "direct" => (
                ProtocolCapabilityLevel::Supported,
                "kernel_builtin",
                network(supported, unsupported),
                network(supported, supported),
                &["tcp", "udp"][..],
                not_applicable,
                &[][..],
            ),
            "block" => (
                ProtocolCapabilityLevel::Supported,
                "kernel_builtin",
                network(unsupported, unsupported),
                network(supported, supported),
                &["tcp", "udp"][..],
                not_applicable,
                &[][..],
            ),
            "mixed" => (
                ProtocolCapabilityLevel::Supported,
                "kernel_builtin",
                network(supported, supported),
                network(unsupported, unsupported),
                &["tcp"][..],
                not_applicable,
                &[][..],
            ),
            _ => (
                ProtocolCapabilityLevel::Experimental,
                "unknown",
                network(unsupported, unsupported),
                network(unsupported, unsupported),
                &[][..],
                unsupported,
                &["protocol_capability_is_not_declared"][..],
            ),
        };

    ProtocolCapabilityDescriptor {
        protocol,
        feature,
        status,
        compatibility_baseline,
        inbound,
        outbound,
        transports,
        mux,
        limitations,
    }
}

pub(crate) fn protocol_capability(descriptor: ProtocolCapabilityDescriptor) -> ProtocolCapability {
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

fn network(
    tcp: ProtocolCapabilityState,
    udp: ProtocolCapabilityState,
) -> ProtocolNetworkCapability {
    ProtocolNetworkCapability { tcp, udp }
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
