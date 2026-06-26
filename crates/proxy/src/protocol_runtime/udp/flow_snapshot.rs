#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProtocolUdpFlowSnapshot {
    Socks5 {
        username: Option<String>,
        password: Option<String>,
    },
    Managed {
        resume: ProtocolUdpFlowResume,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProtocolUdpFlowResume {
    #[cfg(feature = "shadowsocks")]
    Shadowsocks(shadowsocks::ShadowsocksUdpFlowResume),
    #[cfg(feature = "hysteria2")]
    Hysteria2(hysteria2::Hysteria2UdpFlowResume),
    #[cfg(feature = "trojan")]
    Trojan(trojan::TrojanUdpFlowResume),
    #[cfg(feature = "mieru")]
    Mieru(mieru::MieruUdpFlowResume),
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
        Self::Managed {
            resume: ProtocolUdpFlowResume::Shadowsocks(resume),
        }
    }

    #[cfg(feature = "hysteria2")]
    pub(crate) fn hysteria2(resume: hysteria2::Hysteria2UdpFlowResume) -> Self {
        Self::Managed {
            resume: ProtocolUdpFlowResume::Hysteria2(resume),
        }
    }

    #[cfg(feature = "trojan")]
    pub(crate) fn trojan(resume: trojan::TrojanUdpFlowResume) -> Self {
        Self::Managed {
            resume: ProtocolUdpFlowResume::Trojan(resume),
        }
    }

    #[cfg(feature = "mieru")]
    pub(crate) fn mieru(resume: mieru::MieruUdpFlowResume) -> Self {
        Self::Managed {
            resume: ProtocolUdpFlowResume::Mieru(resume),
        }
    }

    pub(crate) fn socks5_relay_auth(&self) -> Option<Socks5RelayAuth<'_>> {
        match self {
            Self::Socks5 { username, password } => Some(Socks5RelayAuth {
                username: username.as_deref(),
                password: password.as_deref(),
            }),
            Self::Managed { .. } => None,
        }
    }
}
