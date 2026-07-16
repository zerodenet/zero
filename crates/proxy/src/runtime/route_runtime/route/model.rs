use std::net::SocketAddr;

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

use super::super::SharedIngressRuntimeServices;

#[derive(Clone)]
pub(crate) struct InboundRouteRuntime {
    pub(super) tcp_runtime: TcpIngressRuntime,
    #[cfg(any(
        feature = "socks5",
        feature = "vless",
        feature = "hysteria2",
        feature = "shadowsocks",
        feature = "trojan",
        feature = "vmess",
        feature = "mieru"
    ))]
    pub(super) udp_runtime: UdpIngressRuntime,
}

impl InboundRouteRuntime {
    pub(crate) fn new(
        shared: SharedIngressRuntimeServices,
        inbound_tag: String,
        source_addr: Option<SocketAddr>,
    ) -> Self {
        let tcp_runtime = shared.tcp_runtime(inbound_tag, source_addr);
        Self {
            #[cfg(any(
                feature = "socks5",
                feature = "vless",
                feature = "hysteria2",
                feature = "shadowsocks",
                feature = "trojan",
                feature = "vmess",
                feature = "mieru"
            ))]
            udp_runtime: shared.udp_runtime(),
            tcp_runtime,
        }
    }
}

#[derive(Clone)]
pub(crate) struct InboundRouteRuntimeFactory {
    pub(super) shared: SharedIngressRuntimeServices,
    pub(super) inbound_tag: String,
}

impl InboundRouteRuntimeFactory {
    pub(crate) fn new(shared: SharedIngressRuntimeServices, inbound_tag: String) -> Self {
        Self {
            shared,
            inbound_tag,
        }
    }
}
