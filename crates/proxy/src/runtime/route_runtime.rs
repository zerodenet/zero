use std::net::SocketAddr;

use zero_core::Session;
use zero_engine::EngineError;

use crate::protocol_registry::TcpRuntimeServices;
use crate::runtime::tcp_ingress::{InboundProtocol, TcpIngressRuntime};
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

    fn tcp_runtime(
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
    fn udp_runtime(&self) -> UdpIngressRuntime {
        self.udp_runtime.clone()
    }
}

#[derive(Clone)]
pub(crate) struct InboundRouteRuntime {
    tcp_runtime: TcpIngressRuntime,
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
        self.udp_runtime.clone()
    }

    #[cfg(any(feature = "vless", feature = "vmess"))]
    pub(crate) fn into_mux_substream_runtime(self) -> MuxSubstreamRuntime {
        MuxSubstreamRuntime::new(self.tcp_runtime.without_source_addr(), self.udp_runtime)
    }

    pub(crate) async fn serve<P>(
        &self,
        session: Session,
        client: P::ClientStream,
        protocol: &P,
    ) -> Result<(), EngineError>
    where
        P: InboundProtocol + 'static,
    {
        self.tcp_runtime.serve(session, client, protocol).await
    }

    #[cfg(any(feature = "socks5", feature = "hysteria2", feature = "mieru"))]
    pub(crate) async fn serve_with_client_response<P, S>(
        &self,
        session: Session,
        client: S,
        response_protocol: P,
    ) -> Result<(), EngineError>
    where
        P: zero_core::InboundClientResponse<S> + Send + Sync,
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + zero_traits::AsyncSocket + Unpin + Send,
    {
        self.tcp_runtime
            .serve_with_client_response(session, client, response_protocol)
            .await
    }

    #[cfg(feature = "vless")]
    pub(crate) async fn relay_recorded_fallback_replay<R>(
        &self,
        fallback: zero_transport::profile::OwnedInboundFallbackProfile,
        replay: R,
    ) -> Result<(), EngineError>
    where
        R: zero_transport::inbound_route::FallbackReplayToUpstream + 'static,
    {
        self.tcp_runtime
            .relay_recorded_fallback_replay(fallback, replay)
            .await
    }
}

#[derive(Clone)]
pub(crate) struct InboundRouteRuntimeFactory {
    shared: SharedIngressRuntimeServices,
    inbound_tag: String,
}

impl InboundRouteRuntimeFactory {
    pub(crate) fn new(shared: SharedIngressRuntimeServices, inbound_tag: String) -> Self {
        Self {
            shared,
            inbound_tag,
        }
    }

    pub(crate) fn inbound_tag(&self) -> &str {
        &self.inbound_tag
    }

    pub(crate) fn for_connection(&self, source_addr: Option<SocketAddr>) -> InboundRouteRuntime {
        InboundRouteRuntime::new(self.shared.clone(), self.inbound_tag.clone(), source_addr)
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
        self.shared.udp_runtime()
    }
}

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

#[cfg(any(feature = "vless", feature = "vmess"))]
#[derive(Clone)]
pub(crate) struct MuxSubstreamRuntime {
    tcp_runtime: TcpIngressRuntime,
    udp_runtime: UdpIngressRuntime,
}

#[cfg(any(feature = "vless", feature = "vmess"))]
impl MuxSubstreamRuntime {
    pub(crate) fn new(tcp_runtime: TcpIngressRuntime, udp_runtime: UdpIngressRuntime) -> Self {
        Self {
            tcp_runtime,
            udp_runtime,
        }
    }

    pub(crate) fn inbound_tag(&self) -> &str {
        self.tcp_runtime.inbound_tag()
    }

    pub(crate) fn udp_runtime(&self) -> UdpIngressRuntime {
        self.udp_runtime.clone()
    }

    pub(crate) async fn open_tcp_upstream(
        &self,
        session: &mut Session,
    ) -> Result<crate::transport::TcpRouteResult, EngineError> {
        self.tcp_runtime.open_tcp_upstream(session).await
    }
}
