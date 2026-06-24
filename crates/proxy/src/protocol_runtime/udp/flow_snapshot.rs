use crate::protocol_runtime::udp::UdpPacketPathCarrier;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProtocolUdpFlowSnapshot {
    #[cfg(feature = "shadowsocks")]
    Shadowsocks {
        password: String,
        datagram_cache_key: String,
        cipher_kind: shadowsocks::CipherKind,
        packet_path_carrier: Option<UdpPacketPathCarrier>,
    },
    #[cfg(feature = "hysteria2")]
    Hysteria2 {
        password: String,
        client_fingerprint: Option<String>,
    },
    #[cfg(feature = "trojan")]
    Trojan {
        password: String,
        sni: Option<String>,
        insecure: bool,
        client_fingerprint: Option<String>,
        relay_chain: bool,
    },
    #[cfg(feature = "mieru")]
    Mieru {
        username: String,
        password: String,
        relay_chain: bool,
    },
}
