use std::net::SocketAddr;

use crate::protocol_registry::TcpRuntimeServices;
use crate::runtime::tcp_ingress::TcpIngressRuntime;
#[cfg(feature = "udp-runtime")]
use crate::runtime::udp_ingress::UdpIngressRuntime;

#[derive(Clone)]
pub(crate) struct SharedIngressRuntimeServices {
    tcp_services: TcpRuntimeServices,
    #[cfg(feature = "udp-runtime")]
    udp_runtime: UdpIngressRuntime,
}

impl SharedIngressRuntimeServices {
    pub(crate) fn new(tcp_services: TcpRuntimeServices) -> Self {
        Self {
            tcp_services: tcp_services.clone(),
            #[cfg(feature = "udp-runtime")]
            udp_runtime: UdpIngressRuntime::new(tcp_services),
        }
    }

    pub(super) fn tcp_runtime(
        &self,
        inbound_tag: String,
        source_addr: Option<SocketAddr>,
    ) -> TcpIngressRuntime {
        TcpIngressRuntime::new(self.tcp_services.clone(), inbound_tag, source_addr)
    }

    #[cfg(feature = "udp-runtime")]
    pub(super) fn udp_runtime(&self) -> UdpIngressRuntime {
        self.udp_runtime.clone()
    }
}
