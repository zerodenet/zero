use zero_traits::{
    ProtocolCapabilityDescriptor, ProtocolCapabilityLevel, ProtocolCapabilityState,
    ProtocolMetadata, ProtocolNetworkCapability,
};

#[derive(Debug, Default, Clone, Copy)]
pub struct VlessProtocol;

impl ProtocolMetadata for VlessProtocol {
    fn descriptor(&self) -> ProtocolCapabilityDescriptor {
        let supported = ProtocolCapabilityState::supported();
        let partial_udp =
            ProtocolCapabilityState::partial(&["udp_relay_chain_final_transport_limited"]);

        ProtocolCapabilityDescriptor {
            protocol: "vless",
            feature: "vless",
            status: ProtocolCapabilityLevel::Partial,
            compatibility_baseline: "xray_core_vless",
            inbound: ProtocolNetworkCapability::new(supported, partial_udp),
            outbound: ProtocolNetworkCapability::new(supported, partial_udp),
            transports: &[
                "tcp",
                "tls",
                "reality",
                "ws",
                "grpc",
                "h2",
                "http_upgrade",
                "split_http",
                "quic",
            ],
            mux: ProtocolCapabilityState::partial(&["mux_udp_is_not_implemented"]),
            limitations: &[
                "udp_relay_chain_final_transport_limited",
                "mux_udp_is_not_implemented",
                "non_reality_tls_fingerprint_passthrough_is_incomplete",
            ],
        }
    }
}
