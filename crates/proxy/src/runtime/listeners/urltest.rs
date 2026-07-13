use tokio::sync::watch;
use tokio::task::JoinSet;
use zero_config::RuntimeConfig;
use zero_engine::EngineError;

use super::super::Proxy;

pub(in crate::runtime) fn reconcile_urltests(
    proxy: &Proxy,
    _new_config: &RuntimeConfig,
    shutdown_rx: &watch::Receiver<bool>,
    urltests: &mut JoinSet<Result<(), EngineError>>,
) {
    let group_ids = proxy.engine.plan().urltest_groups().to_vec();

    for group_id in group_ids {
        let proxy = proxy.clone();
        let shutdown = shutdown_rx.clone();
        urltests.spawn(async move { proxy.run_urltest_group(group_id, shutdown).await });
    }
}
