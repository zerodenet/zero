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

use super::{InboundRouteRuntimeFactory, SharedIngressRuntimeServices};

#[derive(Clone)]
pub(crate) struct InboundListenerRuntime {
    route_factory: InboundRouteRuntimeFactory,
}

impl InboundListenerRuntime {
    pub(crate) fn new(shared: SharedIngressRuntimeServices, inbound_tag: String) -> Self {
        Self {
            route_factory: InboundRouteRuntimeFactory::new(shared, inbound_tag),
        }
    }

    #[cfg(feature = "shadowsocks")]
    pub(crate) fn inbound_tag(&self) -> &str {
        self.route_factory.inbound_tag()
    }

    pub(crate) fn route_factory(&self) -> InboundRouteRuntimeFactory {
        self.route_factory.clone()
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
    pub(crate) fn udp_runtime(&self) -> UdpIngressRuntime {
        self.route_factory.udp_runtime()
    }
}

#[derive(Clone)]
pub(crate) struct InboundListenerRuntimeFactory {
    shared: SharedIngressRuntimeServices,
}

impl InboundListenerRuntimeFactory {
    pub(crate) fn new(shared: SharedIngressRuntimeServices) -> Self {
        Self { shared }
    }

    pub(crate) fn for_inbound(&self, inbound_tag: String) -> InboundListenerRuntime {
        InboundListenerRuntime::new(self.shared.clone(), inbound_tag)
    }
}
