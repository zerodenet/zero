use std::io;

use crate::runtime::tcp_ingress::TcpIngressRuntime;
use crate::transport::{extract_tcp_stream, TcpRouteResult};
use zero_core::Session;
use zero_engine::EngineError;

/// Execute the unified routing and outbound establishment pipeline.
///
/// Caller MUST call `prepare_session` before this to assign a session ID.
pub(crate) async fn dispatch_tcp(
    runtime: &TcpIngressRuntime,
    session: &mut Session,
) -> Result<TcpRouteResult, EngineError> {
    runtime.resolve_fake_ip_target(session).await;
    let action = runtime.route_decision(session);
    let resolved = runtime.resolve_outbound(&action)?;
    let outbound = super::dispatch_tcp_outbound(runtime.runtime_services(), session, resolved)
        .await
        .map_err(|failure| EngineError::Io(io::Error::other(failure.error)))?;
    let mut result = extract_tcp_stream(outbound)?;
    result.route_action = action;
    Ok(result)
}
