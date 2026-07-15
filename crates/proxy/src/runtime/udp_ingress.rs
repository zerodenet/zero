//! Narrow ingress from an accepted inbound UDP packet into [`UdpPipe`].

use zero_core::{InboundUdpDispatch, SessionAuth};
use zero_engine::EngineError;
use zero_traits::DnsResolver;

use crate::protocol_registry::UdpRuntimeServices;
use crate::runtime::pipe::{KernelPipe, UdpPipe, UdpPipeInput};
use crate::runtime::udp_dispatch::UdpDispatch;
use crate::runtime::Proxy;

#[derive(Clone)]
pub(crate) struct UdpIngressRuntime {
    proxy: Proxy,
    services: UdpRuntimeServices,
}

impl UdpIngressRuntime {
    pub(crate) fn from_proxy(proxy: &Proxy) -> Self {
        Self {
            proxy: proxy.clone(),
            services: UdpRuntimeServices::from_proxy(proxy),
        }
    }

    pub(crate) async fn new_dispatch(&self, inbound_tag: &str) -> Result<UdpDispatch, EngineError> {
        UdpDispatch::new(inbound_tag, &self.proxy.protocols).await
    }

    pub(crate) fn services(&self) -> &UdpRuntimeServices {
        &self.services
    }

    pub(crate) async fn resolve_local_dns(&self, domain: &str) {
        let _ = self.proxy.resolver.resolve(domain).await;
    }

    pub(crate) async fn dispatch_inbound_packet(
        &self,
        dispatch: &mut UdpDispatch,
        inbound_dispatch: &InboundUdpDispatch,
        auth: Option<&SessionAuth>,
    ) -> Result<u64, EngineError> {
        dispatch_inbound_udp_packet(&self.proxy, dispatch, inbound_dispatch, auth).await
    }
}

async fn dispatch_inbound_udp_packet(
    proxy: &Proxy,
    dispatch: &mut UdpDispatch,
    inbound_dispatch: &InboundUdpDispatch,
    auth: Option<&SessionAuth>,
) -> Result<u64, EngineError> {
    if !proxy.udp_enabled_for_inbound(dispatch.inbound_tag()) {
        return Err(EngineError::Io(std::io::Error::other(
            "udp disabled for inbound",
        )));
    }

    UdpPipe::new(proxy, dispatch)
        .dispatch(UdpPipeInput::from_inbound_dispatch(inbound_dispatch, auth))
        .await
}
