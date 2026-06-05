use zero_traits::{
    ProtocolCapabilityDescriptor, ProtocolCapabilityLevel, ProtocolCapabilityState,
    ProtocolMetadata, ProtocolNetworkCapability,
};

#[derive(Debug, Default, Clone, Copy)]
pub struct Socks5Protocol;

impl ProtocolMetadata for Socks5Protocol {
    fn descriptor(&self) -> ProtocolCapabilityDescriptor {
        let supported = ProtocolCapabilityState::supported();
        let not_applicable = ProtocolCapabilityState::not_applicable();

        ProtocolCapabilityDescriptor {
            protocol: "socks5",
            feature: "socks5",
            status: ProtocolCapabilityLevel::Supported,
            compatibility_baseline: "rfc_1928_rfc_1929",
            inbound: ProtocolNetworkCapability::new(supported, supported),
            outbound: ProtocolNetworkCapability::new(supported, supported),
            transports: &["tcp"],
            mux: not_applicable,
            limitations: &[],
        }
    }
}
