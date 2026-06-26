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
    pub(crate) fn socks5(resume: socks5::Socks5UdpFlowResume) -> Self {
        Self::Socks5(resume)
    }

    pub(crate) fn as_socks5(&self) -> Option<&socks5::Socks5UdpFlowResume> {
        match self {
            Self::Socks5(resume) => Some(resume),
            #[cfg(feature = "shadowsocks")]
            Self::Shadowsocks(_) => None,
            #[cfg(feature = "hysteria2")]
            Self::Hysteria2(_) => None,
            #[cfg(feature = "trojan")]
            Self::Trojan(_) => None,
            #[cfg(feature = "mieru")]
            Self::Mieru(_) => None,
        }
    }

    #[cfg(feature = "shadowsocks")]
    pub(crate) fn shadowsocks(resume: shadowsocks::ShadowsocksUdpFlowResume) -> Self {
        Self::Shadowsocks(resume)
    }

    #[cfg(feature = "shadowsocks")]
    pub(crate) fn as_shadowsocks(&self) -> Option<&shadowsocks::ShadowsocksUdpFlowResume> {
        match self {
            Self::Shadowsocks(resume) => Some(resume),
            _ => None,
        }
    }

    #[cfg(feature = "shadowsocks")]
    pub(crate) fn into_shadowsocks(self) -> Option<shadowsocks::ShadowsocksUdpFlowResume> {
        match self {
            Self::Shadowsocks(resume) => Some(resume),
            _ => None,
        }
    }

    #[cfg(feature = "hysteria2")]
    pub(crate) fn hysteria2(resume: hysteria2::Hysteria2UdpFlowResume) -> Self {
        Self::Hysteria2(resume)
    }

    #[cfg(feature = "hysteria2")]
    pub(crate) fn as_hysteria2(&self) -> Option<&hysteria2::Hysteria2UdpFlowResume> {
        match self {
            Self::Hysteria2(resume) => Some(resume),
            _ => None,
        }
    }

    #[cfg(feature = "hysteria2")]
    pub(crate) fn into_hysteria2(self) -> Option<hysteria2::Hysteria2UdpFlowResume> {
        match self {
            Self::Hysteria2(resume) => Some(resume),
            _ => None,
        }
    }

    #[cfg(feature = "trojan")]
    pub(crate) fn trojan(resume: trojan::TrojanUdpFlowResume) -> Self {
        Self::Trojan(resume)
    }

    #[cfg(feature = "trojan")]
    pub(crate) fn as_trojan(&self) -> Option<&trojan::TrojanUdpFlowResume> {
        match self {
            Self::Trojan(resume) => Some(resume),
            _ => None,
        }
    }

    #[cfg(feature = "trojan")]
    pub(crate) fn into_trojan(self) -> Option<trojan::TrojanUdpFlowResume> {
        match self {
            Self::Trojan(resume) => Some(resume),
            _ => None,
        }
    }

    #[cfg(feature = "mieru")]
    pub(crate) fn mieru(resume: mieru::MieruUdpFlowResume) -> Self {
        Self::Mieru(resume)
    }

    #[cfg(feature = "mieru")]
    pub(crate) fn as_mieru(&self) -> Option<&mieru::MieruUdpFlowResume> {
        match self {
            Self::Mieru(resume) => Some(resume),
            _ => None,
        }
    }

    #[cfg(feature = "mieru")]
    pub(crate) fn into_mieru(self) -> Option<mieru::MieruUdpFlowResume> {
        match self {
            Self::Mieru(resume) => Some(resume),
            _ => None,
        }
    }
}

impl ProtocolUdpFlowSnapshot {
    pub(crate) fn managed(resume: ProtocolUdpFlowResume) -> Self {
        Self::Managed { resume }
    }

    pub(crate) fn resume(&self) -> &ProtocolUdpFlowResume {
        match self {
            Self::Managed { resume } => resume,
        }
    }
}
