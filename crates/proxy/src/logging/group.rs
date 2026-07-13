use tracing::{debug, info};

pub(crate) fn log_urltest_group_target_changed(
    group_tag: &str,
    previous: Option<&str>,
    selected: &str,
    latency_ms: Option<u64>,
) {
    match previous {
        Some(previous) if previous == selected => debug!(
            group_kind = "url_test",
            group_tag, selected, latency_ms, "outbound group probe refreshed"
        ),
        Some(previous) => info!(
            group_kind = "url_test",
            group_tag, previous, selected, latency_ms, "outbound group target changed"
        ),
        None => info!(
            group_kind = "url_test",
            group_tag, selected, latency_ms, "outbound group target initialized"
        ),
    }
}
