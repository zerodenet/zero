#[cfg(feature = "shadowsocks")]
use crate::protocol_runtime::udp::PacketPathCarrierSnapshot;

#[cfg(feature = "shadowsocks")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PacketPathFlowSnapshot {
    protocol: ProtocolUdpFlowSnapshot,
}

#[cfg(feature = "shadowsocks")]
impl PacketPathFlowSnapshot {
    pub(crate) fn from_protocol(protocol: ProtocolUdpFlowSnapshot) -> Self {
        Self { protocol }
    }

    pub(crate) fn with_packet_path_carrier(
        mut self,
        carrier: Option<PacketPathCarrierSnapshot>,
    ) -> Self {
        self.protocol = self.protocol.with_packet_path_carrier(carrier);
        self
    }

    pub(crate) fn into_protocol_snapshot(self) -> ProtocolUdpFlowSnapshot {
        self.protocol
    }
}

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
        packet_path_carrier: Option<PacketPathCarrierSnapshot>,
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
    pub(crate) fn socks5(username: Option<&str>, password: Option<&str>) -> Self {
        Self::Socks5 {
            username: username.map(ToString::to_string),
            password: password.map(ToString::to_string),
        }
    }

    #[cfg(feature = "shadowsocks")]
    pub(crate) fn shadowsocks(
        password: &str,
        datagram_cache_key: String,
        cipher_kind: shadowsocks::CipherKind,
    ) -> Self {
        Self::Shadowsocks {
            password: password.to_string(),
            datagram_cache_key,
            cipher_kind,
            packet_path_carrier: None,
        }
    }

    #[cfg(feature = "hysteria2")]
    pub(crate) fn hysteria2(password: &str, client_fingerprint: Option<&str>) -> Self {
        Self::Hysteria2 {
            password: password.to_string(),
            client_fingerprint: client_fingerprint.map(ToString::to_string),
        }
    }

    #[cfg(feature = "trojan")]
    pub(crate) fn trojan(
        password: &str,
        sni: Option<&str>,
        insecure: bool,
        client_fingerprint: Option<&str>,
        relay_chain: bool,
    ) -> Self {
        Self::Trojan {
            password: password.to_string(),
            sni: sni.map(ToString::to_string),
            insecure,
            client_fingerprint: client_fingerprint.map(ToString::to_string),
            relay_chain,
        }
    }

    #[cfg(feature = "mieru")]
    pub(crate) fn mieru(username: &str, password: &str, relay_chain: bool) -> Self {
        Self::Mieru {
            username: username.to_string(),
            password: password.to_string(),
            relay_chain,
        }
    }

    #[cfg(feature = "shadowsocks")]
    pub(crate) fn with_packet_path_carrier(
        mut self,
        carrier: Option<PacketPathCarrierSnapshot>,
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
