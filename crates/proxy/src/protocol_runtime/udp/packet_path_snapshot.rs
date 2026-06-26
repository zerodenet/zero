#[cfg(all(feature = "socks5", feature = "shadowsocks"))]
pub(crate) fn socks5_packet_path_carrier_descriptor(
    tag: &str,
    server: &str,
    port: u16,
    username: Option<&str>,
) -> crate::protocol_runtime::udp::PacketPathCarrierDescriptor {
    crate::protocol_runtime::udp::PacketPathCarrierDescriptor {
        cache_key: socks5::udp_cache_key(tag, server, port, username),
        server: server.to_owned(),
        port,
    }
}

#[cfg(feature = "shadowsocks")]
pub(crate) fn shadowsocks_packet_path_carrier_descriptor(
    tag: &str,
    server: &str,
    port: u16,
    cipher: &str,
    password: &str,
) -> crate::protocol_runtime::udp::PacketPathCarrierDescriptor {
    crate::protocol_runtime::udp::PacketPathCarrierDescriptor {
        cache_key: shadowsocks::udp_cache_key(tag, server, port, cipher, password),
        server: server.to_owned(),
        port,
    }
}

#[cfg(feature = "shadowsocks")]
pub(crate) fn shadowsocks_udp_datagram_source<'a>(
    tag: &'a str,
    server: &'a str,
    port: u16,
    cipher: &str,
    password: &'a str,
    cipher_kind: shadowsocks::CipherKind,
) -> crate::protocol_runtime::udp::UdpDatagramSource<'a> {
    let datagram_cache_key = shadowsocks::udp_cache_key(tag, server, port, cipher, password);
    crate::protocol_runtime::udp::UdpDatagramSource {
        descriptor: crate::protocol_runtime::udp::UdpDatagramDescriptor {
            tag,
            server,
            port,
            cache_key: datagram_cache_key.clone(),
        },
        protocol_snapshot: crate::protocol_runtime::udp::ProtocolUdpFlowSnapshot::shadowsocks(
            password,
            datagram_cache_key.clone(),
            cipher_kind,
        ),
        codec: std::sync::Arc::new(shadowsocks::udp_datagram_codec(
            cipher_kind,
            password.as_bytes(),
        )),
    }
}

#[cfg(all(feature = "hysteria2", feature = "shadowsocks"))]
pub(crate) fn hysteria2_packet_path_carrier_descriptor(
    tag: &str,
    server: &str,
    port: u16,
    password: &str,
    client_fingerprint: Option<&str>,
) -> crate::protocol_runtime::udp::PacketPathCarrierDescriptor {
    crate::protocol_runtime::udp::PacketPathCarrierDescriptor {
        cache_key: hysteria2::udp_cache_key(tag, server, port, password, client_fingerprint),
        server: server.to_owned(),
        port,
    }
}
