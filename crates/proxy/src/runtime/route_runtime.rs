use std::net::SocketAddr;

use zero_core::Session;
use zero_engine::EngineError;

use crate::runtime::pipe::{KernelPipe, TcpPipe, TcpPipeInput};
use crate::runtime::tcp_ingress::{serve_inbound, InboundProtocol};
use crate::runtime::udp_ingress::UdpIngressRuntime;
use crate::runtime::Proxy;
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

    pub(crate) fn inbound_tag(&self) -> &str {
        &self.inbound_tag
    }

    pub(crate) fn udp_runtime(&self) -> UdpIngressRuntime {
        UdpIngressRuntime::from_proxy(&self.proxy)
    }

    pub(crate) fn into_mux_substream_runtime(self) -> MuxSubstreamRuntime {
        MuxSubstreamRuntime::new(self.proxy, self.inbound_tag)
    }

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
}

#[derive(Clone)]
pub(crate) struct MuxSubstreamRuntime {
    proxy: Proxy,
    inbound_tag: String,
    udp_runtime: UdpIngressRuntime,
}

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
