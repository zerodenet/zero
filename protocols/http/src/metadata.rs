use zero_traits::{
    ProtocolCapabilityDescriptor, ProtocolCapabilityLevel, ProtocolCapabilityState,
    ProtocolMetadata, ProtocolNetworkCapability,
};

#[derive(Debug, Default, Clone, Copy)]
pub struct HttpConnectProtocol;

impl ProtocolMetadata for HttpConnectProtocol {
    fn descriptor(&self) -> ProtocolCapabilityDescriptor {
        let supported = ProtocolCapabilityState::supported();
        let not_applicable = ProtocolCapabilityState::not_applicable();
        let unsupported = ProtocolCapabilityState::unsupported(&[]);

        ProtocolCapabilityDescriptor {
            protocol: "http",
            feature: "http",
            status: ProtocolCapabilityLevel::Supported,
            compatibility_baseline: "rfc_7231_connect",
            inbound: ProtocolNetworkCapability::new(supported, not_applicable),
            outbound: ProtocolNetworkCapability::new(unsupported, not_applicable),
            transports: &["tcp"],
            mux: not_applicable,
            limitations: &[],
        }
    }
}
