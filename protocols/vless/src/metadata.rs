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
            ProtocolCapabilityState::partial(&["udp_relay_final_hop_not_externally_validated"]);

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
                "xhttp",
            ],
            mux: ProtocolCapabilityState::partial(&["mux_udp_outbound_not_wired"]),
            limitations: &[
                "mux_udp_outbound_not_wired",
                "vless_quic_transport_deprecated_by_xtls",
                "udp_relay_final_hop_not_externally_validated",
            ],
        }
    }
}
