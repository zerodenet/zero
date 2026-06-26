use std::any::Any;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub(crate) enum ProtocolUdpFlowSnapshot {
    Managed { resume: ProtocolUdpFlowResume },
}

trait ProtocolUdpFlowResumeObject: Any + Send + Sync + std::fmt::Debug {
    fn as_any(&self) -> &dyn Any;
}

impl<T> ProtocolUdpFlowResumeObject for T
where
    T: Any + Send + Sync + std::fmt::Debug,
{
    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ProtocolUdpFlowResume {
    inner: Arc<dyn ProtocolUdpFlowResumeObject>,
}

impl ProtocolUdpFlowResume {
    fn new<T>(resume: T) -> Self
    where
        T: Any + Send + Sync + std::fmt::Debug,
    {
        Self {
            inner: Arc::new(resume),
        }
    }

    fn as_resume<T>(&self) -> Option<&T>
    where
        T: Any,
    {
        self.inner.as_any().downcast_ref::<T>()
    }

    fn clone_resume<T>(self) -> Option<T>
    where
        T: Any + Clone,
    {
        self.as_resume::<T>().cloned()
    }

    pub(crate) fn socks5(resume: socks5::Socks5UdpFlowResume) -> Self {
        Self::new(resume)
    }

    pub(crate) fn as_socks5(&self) -> Option<&socks5::Socks5UdpFlowResume> {
        self.as_resume()
    }

    #[cfg(feature = "shadowsocks")]
    pub(crate) fn shadowsocks(resume: shadowsocks::ShadowsocksUdpFlowResume) -> Self {
        Self::new(resume)
    }

    #[cfg(feature = "shadowsocks")]
    pub(crate) fn as_shadowsocks(&self) -> Option<&shadowsocks::ShadowsocksUdpFlowResume> {
        self.as_resume()
    }

    #[cfg(feature = "shadowsocks")]
    pub(crate) fn into_shadowsocks(self) -> Option<shadowsocks::ShadowsocksUdpFlowResume> {
        self.clone_resume()
    }

    #[cfg(feature = "hysteria2")]
    pub(crate) fn hysteria2(resume: hysteria2::Hysteria2UdpFlowResume) -> Self {
        Self::new(resume)
    }

    #[cfg(feature = "hysteria2")]
    pub(crate) fn as_hysteria2(&self) -> Option<&hysteria2::Hysteria2UdpFlowResume> {
        self.as_resume()
    }

    #[cfg(feature = "hysteria2")]
    pub(crate) fn into_hysteria2(self) -> Option<hysteria2::Hysteria2UdpFlowResume> {
        self.clone_resume()
    }

    #[cfg(feature = "trojan")]
    pub(crate) fn trojan(resume: trojan::TrojanUdpFlowResume) -> Self {
        Self::new(resume)
    }

    #[cfg(feature = "trojan")]
    pub(crate) fn as_trojan(&self) -> Option<&trojan::TrojanUdpFlowResume> {
        self.as_resume()
    }

    #[cfg(feature = "trojan")]
    pub(crate) fn into_trojan(self) -> Option<trojan::TrojanUdpFlowResume> {
        self.clone_resume()
    }

    #[cfg(feature = "mieru")]
    pub(crate) fn mieru(resume: mieru::MieruUdpFlowResume) -> Self {
        Self::new(resume)
    }

    #[cfg(feature = "mieru")]
    pub(crate) fn as_mieru(&self) -> Option<&mieru::MieruUdpFlowResume> {
        self.as_resume()
    }

    #[cfg(feature = "mieru")]
    pub(crate) fn into_mieru(self) -> Option<mieru::MieruUdpFlowResume> {
        self.clone_resume()
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
