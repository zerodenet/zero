pub(crate) fn packet_path_carrier_descriptor(
    cache_key: String,
    server: &str,
    port: u16,
) -> crate::protocol_runtime::udp::PacketPathCarrierDescriptor {
    crate::protocol_runtime::udp::PacketPathCarrierDescriptor {
        cache_key,
        server: server.to_owned(),
        port,
    }
}

pub(crate) fn udp_datagram_source<'a>(
    tag: &'a str,
    server: &'a str,
    port: u16,
    cache_key: String,
    codec: std::sync::Arc<
        dyn zero_traits::DatagramCodec<zero_core::Address, Error = zero_core::Error>,
    >,
) -> crate::protocol_runtime::udp::UdpDatagramSource<'a> {
    crate::protocol_runtime::udp::UdpDatagramSource {
        descriptor: crate::protocol_runtime::udp::UdpDatagramDescriptor {
            tag,
            server,
            port,
            cache_key,
        },
        codec,
    }
}
