use crate::runtime::path::{OutboundEndpoint, TcpPathCategory};

/// Runtime-neutral facts about one resolved outbound leaf.
///
/// The proxy runtime uses this for orchestration decisions without matching on
/// concrete protocol variants. Protocol-private fields remain owned by the
/// adapter that claimed the leaf.
#[derive(Debug, Clone)]
pub(crate) struct OutboundLeafRuntime {
    pub(crate) tcp_path: TcpPathCategory,
    #[cfg(feature = "udp-runtime")]
    pub(crate) health_tag: Option<String>,
    pub(crate) endpoint: Option<OutboundEndpoint>,
    pub(crate) kernel_tag: Option<String>,
    #[cfg(feature = "udp-runtime")]
    pub(crate) udp_policy_tag: Option<String>,
}

#[cfg(feature = "udp-runtime")]

impl OutboundLeafRuntime {
    pub(crate) fn proxy(tag: &str, server: &str, port: u16, tcp_path: TcpPathCategory) -> Self {
        Self {
            tcp_path,
            health_tag: Some(tag.to_owned()),
            endpoint: Some(OutboundEndpoint {
                server: server.to_owned(),
                port,
            }),
            kernel_tag: None,
            udp_policy_tag: Some(tag.to_owned()),
        }
    }
}

impl OutboundLeafRuntime {
    pub(crate) fn direct(tag: Option<&str>) -> Self {
        Self {
            tcp_path: TcpPathCategory::Direct,
            #[cfg(feature = "udp-runtime")]
            health_tag: None,
            endpoint: None,
            kernel_tag: tag.map(str::to_owned),
            #[cfg(feature = "udp-runtime")]
            udp_policy_tag: tag.map(str::to_owned),
        }
    }

    pub(crate) fn block(tag: Option<&str>) -> Self {
        Self {
            tcp_path: TcpPathCategory::Block,
            #[cfg(feature = "udp-runtime")]
            health_tag: None,
            endpoint: None,
            kernel_tag: tag.map(str::to_owned),
            #[cfg(feature = "udp-runtime")]
            udp_policy_tag: tag.map(str::to_owned),
        }
    }
}
