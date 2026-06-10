use zero_traits::{
    ProtocolCapabilityDescriptor, ProtocolCapabilityLevel, ProtocolCapabilityState,
    ProtocolMetadata, ProtocolNetworkCapability,
};

#[derive(Debug, Default, Clone, Copy)]
pub struct VmessProtocol;

impl ProtocolMetadata for VmessProtocol {
    fn descriptor(&self) -> ProtocolCapabilityDescriptor {
        let experimental =
            ProtocolCapabilityState::experimental(&["external_interop_coverage_is_incomplete"]);
        let experimental_udp =
            ProtocolCapabilityState::experimental(&["external_interop_coverage_is_incomplete"]);
        let experimental_mux =
            ProtocolCapabilityState::experimental(&["external_interop_coverage_is_incomplete"]);

        ProtocolCapabilityDescriptor {
            protocol: "vmess",
            feature: "vmess",
            status: ProtocolCapabilityLevel::Experimental,
            compatibility_baseline: "xray_core_vmess_aead",
            inbound: ProtocolNetworkCapability::new(experimental, experimental_udp),
            outbound: ProtocolNetworkCapability::new(experimental, experimental_udp),
            transports: &["tcp", "tls", "ws", "grpc"],
            mux: experimental_mux,
            limitations: &["external_interop_coverage_is_incomplete"],
        }
    }
}
