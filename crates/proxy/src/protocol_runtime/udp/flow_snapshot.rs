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

    #[cfg(feature = "shadowsocks")]
    pub(crate) fn shadowsocks(resume: shadowsocks::ShadowsocksUdpFlowResume) -> Self {
        Self::Shadowsocks(resume)
    }

    #[cfg(feature = "hysteria2")]
    pub(crate) fn hysteria2(resume: hysteria2::Hysteria2UdpFlowResume) -> Self {
        Self::Hysteria2(resume)
    }

    #[cfg(feature = "trojan")]
    pub(crate) fn trojan(resume: trojan::TrojanUdpFlowResume) -> Self {
        Self::Trojan(resume)
    }

    #[cfg(feature = "mieru")]
    pub(crate) fn mieru(resume: mieru::MieruUdpFlowResume) -> Self {
        Self::Mieru(resume)
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
