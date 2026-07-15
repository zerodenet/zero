//! Narrow ingress from an accepted inbound UDP packet into [`UdpPipe`].

use std::path::Path;
use std::sync::Arc;

use zero_config::RuntimeConfig;
use zero_core::{InboundUdpDispatch, Session, SessionAuth};
use zero_dns::DnsSystem;
use zero_engine::{Engine, EngineError, ResolvedOutbound, RouteDecision, SessionHandle};
use zero_traits::DnsResolver;

use crate::inventory::ProtocolInventory;
use crate::logging::log_session_accepted;
use crate::protocol_registry::{UdpAdapterContext, UdpRuntimeServices};
use crate::runtime::pipe::{KernelPipe, UdpPipe, UdpPipeInput};
use crate::runtime::tcp_ingress::apply_kernel_rate_limits_from_config;
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};

#[derive(Clone)]
pub(crate) struct UdpIngressRuntime {
    engine: Engine,
    config: Arc<RuntimeConfig>,
    resolver: Arc<DnsSystem>,
    protocols: ProtocolInventory,
    services: UdpRuntimeServices,
}

impl UdpIngressRuntime {
    pub(crate) fn new(
        engine: Engine,
        config: Arc<RuntimeConfig>,
        resolver: Arc<DnsSystem>,
        protocols: ProtocolInventory,
        services: UdpRuntimeServices,
    ) -> Self {
        Self {
            engine,
            config,
            resolver,
            protocols,
            services,
        }
    }

    pub(crate) async fn new_dispatch(&self, inbound_tag: &str) -> Result<UdpDispatch, EngineError> {
        UdpDispatch::new(self.clone(), inbound_tag, &self.protocols).await
    }

    pub(crate) fn services(&self) -> &UdpRuntimeServices {
        &self.services
    }

    pub(crate) fn runtime_services(&self) -> UdpRuntimeServices {
        self.services.clone()
    }

    pub(crate) async fn resolve_local_dns(&self, domain: &str) {
        let _ = self.resolver.resolve(domain).await;
    }

    pub(crate) fn prepare_udp_session(&self, session: &mut Session, inbound_tag: &str) {
        self.engine.prepare_session(session, inbound_tag);
        apply_kernel_rate_limits_from_config(self.config.as_ref(), session, inbound_tag);
    }

    pub(crate) fn track_session(&self, session_id: u64) -> SessionHandle {
        self.engine.track_session(session_id)
    }

    pub(crate) async fn resolve_fake_ip_target(&self, session: &mut Session) {
        use zero_core::Address;
        use zero_traits::IpAddress;

        let ip = match &session.target {
            Address::Ipv4(octets) => IpAddress::V4(*octets),
            Address::Ipv6(octets) => IpAddress::V6(*octets),
            _ => return,
        };
        if let Some(domain) = self.resolver.lookup_fake_ip(&ip).await {
            session.target = Address::Domain(domain);
        }
    }

    pub(crate) fn route_decision(&self, session: &Session) -> RouteDecision {
        self.engine.route_decision_with_inbound(
            &session.target,
            session.sni.as_deref(),
            session.inbound_tag.as_deref(),
        )
    }

    pub(crate) fn resolve_outbound(
        &self,
        action: &RouteDecision,
    ) -> Result<ResolvedOutbound<'static>, EngineError> {
        self.engine
            .resolve_route_decision(action.clone())
            .map(|(resolved, _)| resolved)
    }

    pub(crate) fn log_session_accepted(&self, session: &Session, action: &RouteDecision) {
        log_session_accepted(session, action, self.config.mode.kind());
    }

    pub(crate) fn set_session_outbound(&self, session: &Session) {
        self.engine.set_session_outbound(session);
    }

    #[cfg(any(feature = "socks5", feature = "vless"))]
    pub(crate) fn record_session_inbound_traffic(
        &self,
        session_id: u64,
        traffic: crate::transport::StreamTraffic,
    ) {
        self.services
            .record_session_inbound_traffic(session_id, traffic);
    }

    pub(crate) fn source_dir(&self) -> Option<&Path> {
        self.config.source_dir()
    }

    pub(crate) async fn start_udp_resolved_outbound(
        &self,
        dispatch: &mut UdpDispatch,
        session: &Session,
        resolved: ResolvedOutbound<'_>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        crate::inventory::start_udp_resolved_outbound(
            &self.protocols,
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
        if !self.udp_enabled_for_inbound(dispatch.inbound_tag()) {
            return Err(EngineError::Io(std::io::Error::other(
                "udp disabled for inbound",
            )));
        }

        UdpPipe::new(dispatch)
            .dispatch(UdpPipeInput::from_inbound_dispatch(inbound_dispatch, auth))
            .await
    }

    fn udp_enabled_for_inbound(&self, inbound_tag: &str) -> bool {
        let config = self.engine.config();
        config.runtime.udp.enabled
            && config
                .inbounds
                .iter()
                .find(|inbound| inbound.tag == inbound_tag)
                .map(|inbound| inbound.udp.enabled)
                .unwrap_or(true)
    }
}
