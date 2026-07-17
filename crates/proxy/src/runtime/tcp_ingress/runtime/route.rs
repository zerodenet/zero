use zero_core::Session;
use zero_engine::{EngineError, ResolvedOutbound, RouteDecision};

use crate::logging::log_session_accepted;

use super::model::TcpIngressRuntime;

impl TcpIngressRuntime {
    pub(crate) fn select_http_redirect(&self, session: &Session) -> Option<(u16, String)> {
        crate::runtime::http_redirect::select_redirect_target(
            &self.services.config().route.url_rewrite,
            session,
        )
    }

    pub(crate) fn route_decision(&self, session: &Session) -> RouteDecision {
        self.services.engine().route_decision_with_inbound(
            &session.target,
            session.sni.as_deref(),
            session.inbound_tag.as_deref(),
        )
    }

    pub(crate) fn resolve_outbound(
        &self,
        action: &RouteDecision,
    ) -> Result<ResolvedOutbound<'static>, EngineError> {
        self.services
            .engine()
            .resolve_route_decision(action.clone())
            .map(|(resolved, _)| resolved)
    }

    pub(crate) fn log_session_accepted(&self, session: &Session, action: &RouteDecision) {
        log_session_accepted(session, action, self.services.config().mode.kind());
    }

    pub(crate) fn set_session_outbound(&self, session: &Session) {
        self.services.engine().set_session_outbound(session);
    }
}
