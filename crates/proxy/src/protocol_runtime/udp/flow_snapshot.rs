#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProtocolUdpFlowSnapshot {
    Socks5 {
        username: Option<String>,
        password: Option<String>,
    },
    #[cfg(feature = "shadowsocks")]
    Shadowsocks {
        resume: shadowsocks::ShadowsocksUdpFlowResume,
    },
    #[cfg(feature = "hysteria2")]
    Hysteria2 {
        resume: hysteria2::Hysteria2UdpFlowResume,
    },
    #[cfg(feature = "trojan")]
    Trojan { resume: trojan::TrojanUdpFlowResume },
    #[cfg(feature = "mieru")]
    Mieru { resume: mieru::MieruUdpFlowResume },
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
    pub(crate) fn shadowsocks(resume: shadowsocks::ShadowsocksUdpFlowResume) -> Self {
        Self::Shadowsocks { resume }
    }

    #[cfg(feature = "hysteria2")]
    pub(crate) fn hysteria2(resume: hysteria2::Hysteria2UdpFlowResume) -> Self {
        Self::Hysteria2 { resume }
    }

    #[cfg(feature = "trojan")]
    pub(crate) fn trojan(resume: trojan::TrojanUdpFlowResume) -> Self {
        Self::Trojan { resume }
    }

    #[cfg(feature = "mieru")]
    pub(crate) fn mieru(resume: mieru::MieruUdpFlowResume) -> Self {
        Self::Mieru { resume }
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
