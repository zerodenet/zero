#[cfg(feature = "upstream-association-runtime")]
use tokio::time::{sleep_until, Instant as TokioInstant};

use crate::logging::log_session_finished;
use crate::runtime::udp_flow::sessions::CompletedUdpFlow;

pub(crate) fn log_completed_udp_flow(completed: CompletedUdpFlow) {
    log_session_finished(
        &completed.record,
        completed
            .upstream
            .as_ref()
            .map(|(server, port)| (server.as_str(), *port)),
    );
}

#[cfg(feature = "upstream-association-runtime")]
pub(crate) async fn wait_for_upstream_idle(deadline: Option<TokioInstant>) {
    match deadline {
        Some(deadline) => sleep_until(deadline).await,
        None => std::future::pending::<()>().await,
    }
}
