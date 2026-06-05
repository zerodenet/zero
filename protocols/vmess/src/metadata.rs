use zero_traits::{
    ProtocolCapabilityDescriptor, ProtocolCapabilityLevel, ProtocolCapabilityState,
    ProtocolMetadata, ProtocolNetworkCapability,
};

#[derive(Debug, Default, Clone, Copy)]
pub struct VmessProtocol;

impl ProtocolMetadata for VmessProtocol {
    fn descriptor(&self) -> ProtocolCapabilityDescriptor {
        let unsupported = ProtocolCapabilityState::unsupported(&[]);
        let experimental =
            ProtocolCapabilityState::experimental(&["external_interop_coverage_is_incomplete"]);

        ProtocolCapabilityDescriptor {
            protocol: "vmess",
            feature: "vmess",
            status: ProtocolCapabilityLevel::Experimental,
            compatibility_baseline: "xray_core_vmess_aead",
            inbound: ProtocolNetworkCapability::new(experimental, unsupported),
            outbound: ProtocolNetworkCapability::new(experimental, unsupported),
            transports: &["tcp", "tls", "ws", "grpc"],
            mux: unsupported,
            limitations: &[
                "external_interop_coverage_is_incomplete",
                "vmess_udp_is_not_implemented",
                "cipher_auto_is_not_supported",
            ],
        }
    }
}
