//! Narrow ingress from an accepted inbound UDP packet into [`UdpPipe`].

use zero_core::{InboundUdpDispatch, Session, SessionAuth};
use zero_engine::{EngineError, ResolvedOutbound, RouteDecision, SessionHandle};
use zero_traits::DnsResolver;

use crate::logging::log_session_accepted;
use crate::protocol_registry::{UdpAdapterContext, UdpRuntimeServices};
use crate::runtime::pipe::{KernelPipe, UdpPipe, UdpPipeInput};
use crate::runtime::tcp_ingress::apply_kernel_rate_limits;
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::runtime::Proxy;
use std::path::Path;

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
        UdpDispatch::new(self.clone(), inbound_tag, &self.proxy.protocols).await
    }

    pub(crate) fn services(&self) -> &UdpRuntimeServices {
        &self.services
    }

    pub(crate) fn runtime_services(&self) -> UdpRuntimeServices {
        self.services.clone()
    }

    pub(crate) async fn resolve_local_dns(&self, domain: &str) {
        let _ = self.proxy.resolver.resolve(domain).await;
    }

    pub(crate) fn prepare_udp_session(&self, session: &mut Session, inbound_tag: &str) {
        self.proxy.prepare_session(session, inbound_tag, None);
        apply_kernel_rate_limits(&self.proxy, session, inbound_tag);
    }

    pub(crate) fn track_session(&self, session_id: u64) -> SessionHandle {
        self.proxy.track_session(session_id)
    }

    pub(crate) async fn resolve_fake_ip_target(&self, session: &mut Session) {
        self.proxy.resolve_fake_ip_target(session).await;
    }

    pub(crate) fn route_decision(&self, session: &Session) -> RouteDecision {
        self.proxy.route_decision(session)
    }

    pub(crate) fn resolve_outbound(
        &self,
        action: &RouteDecision,
    ) -> Result<ResolvedOutbound<'static>, EngineError> {
        self.proxy
            .resolve_outbound(action)
            .map(|(resolved, _)| resolved)
    }

    pub(crate) fn log_session_accepted(&self, session: &Session, action: &RouteDecision) {
        log_session_accepted(session, action, self.proxy.config.mode.kind());
    }

    pub(crate) fn set_session_outbound(&self, session: &Session) {
        self.proxy.set_session_outbound(session);
    }

    pub(crate) fn source_dir(&self) -> Option<&Path> {
        self.proxy.config.source_dir()
    }

    pub(crate) async fn start_udp_resolved_outbound(
        &self,
        dispatch: &mut UdpDispatch,
        session: &Session,
        resolved: ResolvedOutbound<'_>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        crate::inventory::start_udp_resolved_outbound(
            &self.proxy.protocols,
            dispatch,
            UdpAdapterContext::new(self.source_dir(), self.runtime_services()),
            session,
            resolved,
            payload,
        )
        .await
    }

    pub(crate) async fn dispatch_inbound_packet(
        &self,
        dispatch: &mut UdpDispatch,
        inbound_dispatch: &InboundUdpDispatch,
        auth: Option<&SessionAuth>,
    ) -> Result<u64, EngineError> {
        if !self.proxy.udp_enabled_for_inbound(dispatch.inbound_tag()) {
            return Err(EngineError::Io(std::io::Error::other(
                "udp disabled for inbound",
            )));
        }

        UdpPipe::new(dispatch)
            .dispatch(UdpPipeInput::from_inbound_dispatch(inbound_dispatch, auth))
            .await
    }
}
