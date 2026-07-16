use std::net::SocketAddr;

use zero_core::Session;
use zero_engine::EngineError;

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

#[cfg(any(feature = "vless", feature = "vmess"))]
use super::MuxSubstreamRuntime;
use super::SharedIngressRuntimeServices;

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
