use std::time::Duration;

use tracing::{debug, info, warn};

pub(crate) fn log_udp_upstream_association_created(
    inbound_tag: &str,
    outbound_tag: &str,
    server: &str,
    port: u16,
    idle_timeout: Duration,
) {
    info!(
        inbound_tag,
        outbound_tag,
        protocol = "udp_upstream",
        upstream_server = server,
        upstream_port = port,
        idle_timeout_seconds = idle_timeout.as_secs(),
        "created upstream UDP association"
    );
}

pub(crate) fn log_udp_upstream_association_reused(
    inbound_tag: &str,
    outbound_tag: &str,
    server: &str,
    port: u16,
) {
    debug!(
        inbound_tag,
        outbound_tag,
        protocol = "udp_upstream",
        upstream_server = server,
        upstream_port = port,
        "reused upstream UDP association"
    );
}

pub(crate) fn log_udp_upstream_association_idle_timeout(
    inbound_tag: &str,
    outbound_tag: &str,
    server: &str,
    port: u16,
    idle_timeout: Duration,
) {
    info!(
        inbound_tag,
        outbound_tag,
        protocol = "udp_upstream",
        upstream_server = server,
        upstream_port = port,
        idle_timeout_seconds = idle_timeout.as_secs(),
        "closed idle upstream UDP association"
    );
}

pub(crate) fn log_udp_upstream_association_dropped(
    inbound_tag: &str,
    outbound_tag: &str,
    server: &str,
    port: u16,
    error: &impl std::fmt::Display,
) {
    warn!(inbound_tag, outbound_tag, protocol = "udp_upstream", upstream_server = server,
        upstream_port = port, error = %error, "dropped upstream UDP association");
}
