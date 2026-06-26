#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProtocolUdpFlowSnapshot {
    Managed { resume: ProtocolUdpFlowResume },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProtocolUdpFlowResume {
    Socks5(socks5::Socks5UdpFlowResume),
    #[cfg(feature = "shadowsocks")]
    Shadowsocks(shadowsocks::ShadowsocksUdpFlowResume),
    #[cfg(feature = "hysteria2")]
    Hysteria2(hysteria2::Hysteria2UdpFlowResume),
    #[cfg(feature = "trojan")]
    Trojan(trojan::TrojanUdpFlowResume),
    #[cfg(feature = "mieru")]
    Mieru(mieru::MieruUdpFlowResume),
}

impl ProtocolUdpFlowResume {
    #[cfg(feature = "shadowsocks")]
    pub(crate) fn shadowsocks(&self) -> Option<&shadowsocks::ShadowsocksUdpFlowResume> {
        match self {
            Self::Shadowsocks(resume) => Some(resume),
            _ => None,
        }
    }

    #[cfg(feature = "hysteria2")]
    pub(crate) fn hysteria2(&self) -> Option<&hysteria2::Hysteria2UdpFlowResume> {
        match self {
            Self::Hysteria2(resume) => Some(resume),
            _ => None,
        }
    }

    #[cfg(feature = "trojan")]
    pub(crate) fn trojan(&self) -> Option<&trojan::TrojanUdpFlowResume> {
        match self {
            Self::Trojan(resume) => Some(resume),
            _ => None,
        }
    }

    #[cfg(feature = "mieru")]
    pub(crate) fn mieru(&self) -> Option<&mieru::MieruUdpFlowResume> {
        match self {
            Self::Mieru(resume) => Some(resume),
            _ => None,
        }
    }
}

pub(crate) struct Socks5RelayAuth<'a> {
    pub(crate) username: Option<&'a str>,
    pub(crate) password: Option<&'a str>,
}

impl ProtocolUdpFlowSnapshot {
    pub(crate) fn managed(resume: ProtocolUdpFlowResume) -> Self {
        Self::Managed { resume }
    }

    pub(crate) fn socks5(username: Option<&str>, password: Option<&str>) -> Self {
        Self::Managed {
            resume: ProtocolUdpFlowResume::Socks5(socks5::Socks5UdpFlowResume::new(
                username, password,
            )),
        }
    }

    pub(crate) fn socks5_relay_auth(&self) -> Option<Socks5RelayAuth<'_>> {
        match self {
            Self::Managed {
                resume: ProtocolUdpFlowResume::Socks5(resume),
            } => Some(Socks5RelayAuth {
                username: resume.username(),
                password: resume.password(),
            }),
            Self::Managed { .. } => None,
        }
    }
}
