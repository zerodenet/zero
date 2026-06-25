#[cfg(feature = "shadowsocks")]
use crate::protocol_runtime::udp::UdpPacketPathCarrier;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProtocolUdpFlowSnapshot {
    Socks5 {
        username: Option<String>,
        password: Option<String>,
    },
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

pub(crate) struct Socks5RelayAuth<'a> {
    pub(crate) username: Option<&'a str>,
    pub(crate) password: Option<&'a str>,
}

impl ProtocolUdpFlowSnapshot {
    #[cfg(feature = "shadowsocks")]
    pub(crate) fn with_packet_path_carrier(
        mut self,
        carrier: Option<UdpPacketPathCarrier>,
    ) -> Self {
        if let Self::Shadowsocks {
            packet_path_carrier,
            ..
        } = &mut self
        {
            *packet_path_carrier = carrier;
        }
        self
    }

    pub(crate) fn socks5_relay_auth(&self) -> Option<Socks5RelayAuth<'_>> {
        match self {
            Self::Socks5 { username, password } => Some(Socks5RelayAuth {
                username: username.as_deref(),
                password: password.as_deref(),
            }),
            #[cfg(feature = "shadowsocks")]
            Self::Shadowsocks { .. } => None,
            #[cfg(feature = "hysteria2")]
            Self::Hysteria2 { .. } => None,
            #[cfg(feature = "trojan")]
            Self::Trojan { .. } => None,
            #[cfg(feature = "mieru")]
            Self::Mieru { .. } => None,
        }
    }
}
