use std::net::SocketAddr;

use zero_core::Session;
use zero_engine::EngineError;

#[cfg(any(feature = "vless", feature = "vmess"))]
use crate::runtime::pipe::{KernelPipe, TcpPipe, TcpPipeInput};
use crate::runtime::tcp_ingress::{serve_inbound, InboundProtocol};
use crate::runtime::Proxy;
#[cfg(any(feature = "vless", feature = "vmess"))]
use crate::transport::TcpRouteResult;

#[derive(Clone)]
pub(crate) struct TcpIngressRuntime {
    proxy: Proxy,
    inbound_tag: String,
    source_addr: Option<SocketAddr>,
}

impl TcpIngressRuntime {
    pub(crate) fn new(proxy: Proxy, inbound_tag: String, source_addr: Option<SocketAddr>) -> Self {
        Self {
            proxy,
            inbound_tag,
            source_addr,
        }
    }

    pub(crate) fn inbound_tag(&self) -> &str {
        &self.inbound_tag
    }

    pub(crate) fn source_addr(&self) -> Option<SocketAddr> {
        self.source_addr
    }

    #[cfg(any(feature = "vless", feature = "vmess"))]
    pub(crate) fn without_source_addr(&self) -> Self {
        Self {
            proxy: self.proxy.clone(),
            inbound_tag: self.inbound_tag.clone(),
            source_addr: None,
        }
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

    #[cfg(feature = "vless")]
    pub(crate) async fn relay_recorded_fallback_replay<R>(
        &self,
        fallback: zero_transport::profile::OwnedInboundFallbackProfile,
        replay: R,
    ) -> Result<(), EngineError>
    where
        R: zero_transport::inbound_route::FallbackReplayToUpstream + 'static,
    {
        crate::runtime::inbound_fallback::relay_recorded_fallback_replay(
            self.proxy.clone(),
            fallback,
            replay,
        )
        .await
    }

    #[cfg(any(feature = "vless", feature = "vmess"))]
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
