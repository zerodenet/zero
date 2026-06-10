use zero_traits::{
    ProtocolCapabilityDescriptor, ProtocolCapabilityLevel, ProtocolCapabilityState,
    ProtocolMetadata, ProtocolNetworkCapability,
};

#[derive(Debug, Default, Clone, Copy)]
pub struct VmessProtocol;

impl ProtocolMetadata for VmessProtocol {
    fn descriptor(&self) -> ProtocolCapabilityDescriptor {
        let partial = ProtocolCapabilityState::partial(&[
            "external_interop_coverage_is_incomplete",
            "cipher_zero_mainstream_compatibility_is_incomplete",
        ]);
        let partial_udp = ProtocolCapabilityState::partial(&[
            "external_interop_coverage_is_incomplete",
            "cipher_zero_mainstream_compatibility_is_incomplete",
        ]);
        let partial_mux = ProtocolCapabilityState::partial(&[
            "external_interop_coverage_is_incomplete",
            "cipher_zero_mainstream_compatibility_is_incomplete",
        ]);

        ProtocolCapabilityDescriptor {
            protocol: "vmess",
            feature: "vmess",
            status: ProtocolCapabilityLevel::Partial,
            compatibility_baseline: "xray_core_vmess_aead",
            inbound: ProtocolNetworkCapability::new(partial, partial_udp),
            outbound: ProtocolNetworkCapability::new(partial, partial_udp),
            transports: &["tcp", "tls", "ws", "grpc"],
            mux: partial_mux,
            limitations: &[
                "external_interop_coverage_is_incomplete",
                "cipher_zero_mainstream_compatibility_is_incomplete",
            ],
        }
    }
}
