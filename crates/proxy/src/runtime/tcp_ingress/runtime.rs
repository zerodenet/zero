use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use zero_config::RuntimeConfig;
use zero_core::{Address, Session};
use zero_dns::DnsSystem;
use zero_engine::{Engine, EngineError, ResolvedOutbound, RouteDecision, SessionHandle};

use super::contract::InboundProtocol;
use super::lifecycle::{apply_kernel_rate_limits_from_config, serve_inbound};
use crate::logging::log_session_accepted;
use crate::protocol_registry::TcpRuntimeServices;
#[cfg(any(feature = "vless", feature = "vmess"))]
use crate::runtime::pipe::{KernelPipe, TcpPipe, TcpPipeInput};
#[cfg(any(feature = "vless", feature = "vmess"))]
use crate::transport::TcpRouteResult;

#[derive(Clone)]
pub(crate) struct TcpIngressRuntime {
    engine: Engine,
    config: Arc<RuntimeConfig>,
    resolver: Arc<DnsSystem>,
    services: TcpRuntimeServices,
    inbound_tag: String,
    source_addr: Option<SocketAddr>,
}

impl TcpIngressRuntime {
    pub(crate) fn new(
        engine: Engine,
        config: Arc<RuntimeConfig>,
        resolver: Arc<DnsSystem>,
        services: TcpRuntimeServices,
        inbound_tag: String,
        source_addr: Option<SocketAddr>,
    ) -> Self {
        Self {
            engine,
            config,
            resolver,
            services,
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
            engine: self.engine.clone(),
            config: self.config.clone(),
            resolver: self.resolver.clone(),
            services: self.services.clone(),
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
            &self.config.route.url_rewrite,
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
        serve_inbound(self, session, client, protocol).await
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
        super::lifecycle::serve_inbound_with_client_response(
            self,
            session,
            client,
            response_protocol,
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
            self.runtime_services(),
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
        self.prepare_session(session);
        TcpPipe::new(self).dispatch(TcpPipeInput { session }).await
    }

    pub(crate) fn runtime_services(&self) -> TcpRuntimeServices {
        self.services.clone()
    }

    pub(crate) async fn resolve_fake_ip_target(&self, session: &mut Session) {
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

    pub(crate) fn apply_url_rewrite(&self, session: &mut Session) {
        let rules = &self.config.route.url_rewrite;
        if rules.is_empty() {
            return;
        }
        let Address::Domain(ref domain) = session.target else {
            return;
        };
        for rule in rules {
            if let Some(ref from) = rule.from {
                if from == domain {
                    session.target = Address::Domain(rule.to.clone());
                    return;
                }
            }
            if let Some(ref pattern) = &rule.from_regex {
                if let Ok(re) = regex::Regex::new(pattern) {
                    if re.is_match(domain) {
                        let result = re.replace(domain, &rule.to);
                        session.target = Address::Domain(result.to_string());
                        return;
                    }
                }
            }
        }
    }

    pub(crate) fn apply_kernel_rate_limits(&self, session: &mut Session) {
        apply_kernel_rate_limits_from_config(self.config.as_ref(), session, &self.inbound_tag);
    }

    pub(crate) fn prepare_session(&self, session: &mut Session) {
        if let Some(addr) = self.source_addr {
            session.source_ip = Some(match addr.ip() {
                std::net::IpAddr::V4(v4) => Address::Ipv4(v4.octets()),
                std::net::IpAddr::V6(v6) => Address::Ipv6(v6.octets()),
            });
            session.source_port = Some(addr.port());
        }
        self.engine.prepare_session(session, &self.inbound_tag);

        if let Some(addr) = self.source_addr {
            if let Some(info) = crate::process_lookup::lookup_process(addr) {
                session.process_id = Some(info.pid);
                session.process_name = Some(info.name);
            }
        }
    }

    pub(crate) fn track_session(&self, session_id: u64) -> SessionHandle {
        self.engine.track_session(session_id)
    }

    pub(crate) fn log_session_accepted(&self, session: &Session, action: &RouteDecision) {
        log_session_accepted(session, action, self.config.mode.kind());
    }

    pub(crate) fn set_session_outbound(&self, session: &Session) {
        self.engine.set_session_outbound(session);
    }

    pub(crate) fn idle_timeout(&self) -> Duration {
        Duration::from_secs(
            self.config
                .inbounds
                .iter()
                .find(|i| i.tag == self.inbound_tag)
                .and_then(|i| i.idle_timeout_secs)
                .unwrap_or(300),
        )
    }
}
