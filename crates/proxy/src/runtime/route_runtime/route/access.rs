use std::net::SocketAddr;

#[cfg(any(feature = "vless", feature = "vmess"))]
use super::super::MuxSubstreamRuntime;
use super::model::{InboundRouteRuntime, InboundRouteRuntimeFactory};
#[cfg(feature = "udp-runtime")]
use crate::runtime::udp_ingress::UdpIngressRuntime;

impl InboundRouteRuntime {
    pub(crate) fn inbound_tag(&self) -> &str {
        self.tcp_runtime.inbound_tag()
    }

    pub(crate) fn source_addr(&self) -> Option<SocketAddr> {
        self.tcp_runtime.source_addr()
    }

    #[cfg(feature = "http")]
    pub(crate) fn select_http_redirect(
        &self,
        session: &zero_core::Session,
    ) -> Option<(u16, String)> {
        self.tcp_runtime.select_http_redirect(session)
    }

    #[cfg(feature = "udp-runtime")]
    pub(crate) fn udp_runtime(&self) -> UdpIngressRuntime {
        self.udp_runtime.clone()
    }

    #[cfg(any(feature = "vless", feature = "vmess"))]
    pub(crate) fn into_mux_substream_runtime(self) -> MuxSubstreamRuntime {
        MuxSubstreamRuntime::new(self.tcp_runtime.without_source_addr(), self.udp_runtime)
    }
}

impl InboundRouteRuntimeFactory {
    pub(crate) fn inbound_tag(&self) -> &str {
        &self.inbound_tag
    }

    pub(crate) fn for_connection(&self, source_addr: Option<SocketAddr>) -> InboundRouteRuntime {
        InboundRouteRuntime::new(self.shared.clone(), self.inbound_tag.clone(), source_addr)
    }

    #[cfg(feature = "udp-runtime")]
    pub(crate) fn udp_runtime(&self) -> UdpIngressRuntime {
        self.shared.udp_runtime()
    }
}
