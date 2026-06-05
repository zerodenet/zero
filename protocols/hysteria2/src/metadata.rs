use zero_traits::{
    ProtocolCapabilityDescriptor, ProtocolCapabilityLevel, ProtocolCapabilityState,
    ProtocolMetadata, ProtocolNetworkCapability,
};

#[derive(Debug, Default, Clone, Copy)]
pub struct Hysteria2Protocol;

impl ProtocolMetadata for Hysteria2Protocol {
    fn descriptor(&self) -> ProtocolCapabilityDescriptor {
        let unsupported = ProtocolCapabilityState::unsupported(&[]);
        let supported = ProtocolCapabilityState::supported();
        let partial = ProtocolCapabilityState::partial(&[
            "udp_relay_chain_is_not_supported",
            "external_interop_coverage_is_incomplete",
        ]);

        ProtocolCapabilityDescriptor {
            protocol: "hysteria2",
            feature: "hysteria2",
            status: ProtocolCapabilityLevel::Partial,
            compatibility_baseline: "hysteria",
            inbound: ProtocolNetworkCapability::new(supported, partial),
            outbound: ProtocolNetworkCapability::new(supported, partial),
            transports: &["quic"],
            mux: unsupported,
            limitations: &[
                "udp_relay_chain_is_not_supported",
                "external_interop_coverage_is_incomplete",
            ],
        }
    }
}
