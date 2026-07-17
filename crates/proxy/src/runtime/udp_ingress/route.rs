use zero_core::Session;
use zero_engine::{EngineError, ResolvedOutbound, RouteDecision};

use super::model::UdpIngressRuntime;
use crate::logging::log_session_accepted;
use crate::protocol_registry::UdpAdapterContext;
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};

impl UdpIngressRuntime {
    pub(crate) fn route_decision(&self, session: &Session) -> RouteDecision {
        self.tcp_services.engine().route_decision_with_inbound(
            &session.target,
            session.sni.as_deref(),
            session.inbound_tag.as_deref(),
        )
    }

    pub(crate) fn resolve_outbound(
        &self,
        action: &RouteDecision,
    ) -> Result<ResolvedOutbound<'static>, EngineError> {
        self.tcp_services
            .engine()
            .resolve_route_decision(action.clone())
            .map(|(resolved, _)| resolved)
    }

    pub(crate) fn log_session_accepted(&self, session: &Session, action: &RouteDecision) {
        log_session_accepted(session, action, self.tcp_services.config().mode.kind());
    }

    pub(crate) fn set_session_outbound(&self, session: &Session) {
        self.tcp_services.engine().set_session_outbound(session);
    }

    pub(crate) async fn start_udp_resolved_outbound(
        &self,
        dispatch: &mut UdpDispatch,
        session: &Session,
        resolved: ResolvedOutbound<'_>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        crate::runtime::udp_dispatch::start_udp_resolved_outbound(
            dispatch,
            UdpAdapterContext::new(self.tcp_services.config(), self.runtime_services()),
            session,
            resolved,
            payload,
        )
        .await
    }
}
