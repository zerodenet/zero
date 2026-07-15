use std::net::SocketAddr;

use zero_core::Session;
use zero_engine::EngineError;

#[cfg(any(feature = "vless", feature = "vmess"))]
use crate::runtime::pipe::{KernelPipe, TcpPipe, TcpPipeInput};
use crate::runtime::tcp_ingress::{serve_inbound, InboundProtocol};
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
use crate::runtime::Proxy;
#[cfg(any(feature = "vless", feature = "vmess"))]
use crate::transport::TcpRouteResult;

#[derive(Clone)]
pub(crate) struct InboundRouteRuntime {
    proxy: Proxy,
    inbound_tag: String,
    source_addr: Option<SocketAddr>,
}

impl InboundRouteRuntime {
    pub(crate) fn new(proxy: Proxy, inbound_tag: String, source_addr: Option<SocketAddr>) -> Self {
        Self {
            proxy,
            inbound_tag,
            source_addr,
        }
    }

    #[cfg(any(
        feature = "socks5",
        feature = "vless",
        feature = "trojan",
        feature = "vmess",
        feature = "mieru"
    ))]
    pub(crate) fn inbound_tag(&self) -> &str {
        &self.inbound_tag
    }

    #[cfg(feature = "http")]
    pub(crate) fn select_http_redirect(
        &self,
        session: &zero_core::Session,
    ) -> Option<(u16, String)> {
        crate::runtime::http_redirect::select_redirect_target(
            &self.proxy.config.route.url_rewrite,
            session,
        )
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
        UdpIngressRuntime::from_proxy(&self.proxy)
    }

    #[cfg(any(feature = "vless", feature = "vmess"))]
    pub(crate) fn into_mux_substream_runtime(self) -> MuxSubstreamRuntime {
        MuxSubstreamRuntime::new(self.proxy, self.inbound_tag)
    }

    #[cfg(feature = "vless")]
    pub(crate) fn fallback_proxy(&self) -> Proxy {
        self.proxy.clone()
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
        serve_inbound(
            &self.proxy,
            session,
            client,
            protocol,
            &self.inbound_tag,
            self.source_addr,
        )
        .await
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
        crate::runtime::tcp_ingress::serve_inbound_with_client_response(
            &self.proxy,
            session,
            client,
            response_protocol,
            &self.inbound_tag,
            self.source_addr,
        )
        .await
    }
}

#[cfg(any(feature = "vless", feature = "vmess"))]
#[derive(Clone)]
pub(crate) struct MuxSubstreamRuntime {
    proxy: Proxy,
    inbound_tag: String,
    udp_runtime: UdpIngressRuntime,
}

#[cfg(any(feature = "vless", feature = "vmess"))]
impl MuxSubstreamRuntime {
    pub(crate) fn new(proxy: Proxy, inbound_tag: String) -> Self {
        Self {
            udp_runtime: UdpIngressRuntime::from_proxy(&proxy),
            proxy,
            inbound_tag,
        }
    }

    pub(crate) fn inbound_tag(&self) -> &str {
        &self.inbound_tag
    }

    pub(crate) fn udp_runtime(&self) -> UdpIngressRuntime {
        self.udp_runtime.clone()
    }

    pub(crate) async fn open_tcp_upstream(
        &self,
        session: &mut Session,
    ) -> Result<TcpRouteResult, EngineError> {
        self.proxy.prepare_session(session, &self.inbound_tag, None);
        TcpPipe::new(&self.proxy)
            .dispatch(TcpPipeInput { session })
            .await
    }
}
