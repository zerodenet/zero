use zero_traits::{
    ProtocolCapabilityDescriptor, ProtocolCapabilityLevel, ProtocolCapabilityState,
    ProtocolMetadata, ProtocolNetworkCapability,
};

#[derive(Debug, Default, Clone, Copy)]
pub struct TrojanProtocol;

impl ProtocolMetadata for TrojanProtocol {
    fn descriptor(&self) -> ProtocolCapabilityDescriptor {
        let unsupported = ProtocolCapabilityState::unsupported(&[]);
        let supported = ProtocolCapabilityState::supported();
        let partial = ProtocolCapabilityState::partial(&[
            "external_interop_coverage_is_incomplete",
            "relay_stream_tls_client_fingerprint_is_not_supported",
        ]);

        ProtocolCapabilityDescriptor {
            protocol: "trojan",
            feature: "trojan",
            status: ProtocolCapabilityLevel::Partial,
            compatibility_baseline: "trojan_go",
            inbound: ProtocolNetworkCapability::new(supported, partial),
            outbound: ProtocolNetworkCapability::new(supported, partial),
            transports: &["tcp", "tls"],
            mux: unsupported,
            limitations: &[
                "external_interop_coverage_is_incomplete",
                "relay_stream_tls_client_fingerprint_is_not_supported",
            ],
        }
    }
}
