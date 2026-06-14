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

        ProtocolCapabilityDescriptor {
            protocol: "shadowsocks",
            feature: "shadowsocks",
            status: ProtocolCapabilityLevel::Partial,
            compatibility_baseline: "shadowsocks_rust_sip022",
            inbound: ProtocolNetworkCapability::new(supported, supported),
            outbound: ProtocolNetworkCapability::new(supported, supported),
            transports: &["tcp", "udp"],
            mux: unsupported,
            limitations: &[
                "shadowsocks_2022_udp_server_response_context_is_not_implemented",
                "shadowsocks_2022_tcp_replay_protection_lacks_salt_pool",
            ],
        }
    }
}
