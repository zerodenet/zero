use std::io;

use crate::protocol_registry::TcpRuntimeServices;
use crate::runtime::Proxy;
use crate::transport::{extract_tcp_stream, TcpRouteResult};
use zero_core::Session;
use zero_engine::EngineError;

impl Proxy {
    /// Execute the unified routing and outbound establishment pipeline.
    ///
    /// Caller MUST call `prepare_session` before this to assign a session ID.
    pub(crate) async fn dispatch_tcp(
        &self,
        session: &mut Session,
    ) -> Result<TcpRouteResult, EngineError> {
        self.resolve_fake_ip_target(session).await;
        let action = self.route_decision(session);
        let (resolved, _plan) = self.resolve_outbound(&action)?;
        let outbound = crate::inventory::dispatch_tcp_outbound(
            TcpRuntimeServices::from_proxy(self),
            session,
            resolved,
        )
        .await
        .map_err(|f| EngineError::Io(io::Error::other(f.error)))?;
        let mut result = extract_tcp_stream(outbound)?;
        result.route_action = action;
        Ok(result)
    }
}
