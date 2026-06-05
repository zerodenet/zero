use zero_traits::{
    ProtocolCapabilityDescriptor, ProtocolCapabilityLevel, ProtocolCapabilityState,
    ProtocolMetadata, ProtocolNetworkCapability,
};

#[derive(Debug, Default, Clone, Copy)]
pub struct ShadowsocksProtocol;

impl ProtocolMetadata for ShadowsocksProtocol {
    fn descriptor(&self) -> ProtocolCapabilityDescriptor {
        let unsupported = ProtocolCapabilityState::unsupported(&[]);
        let supported = ProtocolCapabilityState::supported();
        let partial = ProtocolCapabilityState::partial(&[
            "udp_relay_chain_is_not_supported",
            "external_interop_coverage_is_incomplete",
        ]);

        ProtocolCapabilityDescriptor {
            protocol: "shadowsocks",
            feature: "shadowsocks",
            status: ProtocolCapabilityLevel::Partial,
            compatibility_baseline: "shadowsocks_rust_sip022",
            inbound: ProtocolNetworkCapability::new(supported, partial),
            outbound: ProtocolNetworkCapability::new(supported, partial),
            transports: &["tcp", "udp"],
            mux: unsupported,
            limitations: &[
                "udp_relay_chain_is_not_supported",
                "external_interop_coverage_is_incomplete",
            ],
        }
    }
}
