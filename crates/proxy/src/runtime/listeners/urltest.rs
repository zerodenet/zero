use tokio::sync::watch;
use tokio::task::JoinSet;
use zero_engine::EngineError;

use crate::groups::UrlTestRuntime;

pub(in crate::runtime) async fn reconcile_urltests(
    runtime: &UrlTestRuntime,
    shutdown_rx: &watch::Receiver<bool>,
    urltests: &mut JoinSet<Result<(), EngineError>>,
) {
    urltests.abort_all();
    while urltests.join_next().await.is_some() {}
    runtime.clear_probe_triggers();

    let group_ids = runtime.group_ids();

    for group_id in group_ids {
        let runtime = runtime.clone();
        let shutdown = shutdown_rx.clone();
        urltests.spawn(async move { runtime.run_urltest_group(group_id, shutdown).await });
    }
}
