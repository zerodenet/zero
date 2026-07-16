use std::net::SocketAddr;

use crate::protocol_registry::TcpRuntimeServices;
use crate::runtime::tcp_ingress::TcpIngressRuntime;
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
use crate::runtime::udp_ingress::UdpIngressRuntime;

#[derive(Clone)]
pub(crate) struct SharedIngressRuntimeServices {
    tcp_services: TcpRuntimeServices,
    #[cfg(any(
        feature = "socks5",
        feature = "vless",
        feature = "hysteria2",
        feature = "shadowsocks",
        feature = "trojan",
        feature = "vmess",
        feature = "mieru"
    ))]
    udp_runtime: UdpIngressRuntime,
}

impl SharedIngressRuntimeServices {
    pub(crate) fn new(tcp_services: TcpRuntimeServices) -> Self {
        Self {
            tcp_services: tcp_services.clone(),
            #[cfg(any(
                feature = "socks5",
                feature = "vless",
                feature = "hysteria2",
                feature = "shadowsocks",
                feature = "trojan",
                feature = "vmess",
                feature = "mieru"
            ))]
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

    #[cfg(any(
        feature = "socks5",
        feature = "vless",
        feature = "hysteria2",
        feature = "shadowsocks",
        feature = "trojan",
        feature = "vmess",
        feature = "mieru"
    ))]
    pub(super) fn udp_runtime(&self) -> UdpIngressRuntime {
        self.udp_runtime.clone()
    }
}
