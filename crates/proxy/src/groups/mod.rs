mod urltest;

/// Re-exported so `ProxyHandle` can fall back to it for
/// `diagnostics.probe_outbound` when the caller omits `url`.
pub(crate) use urltest::{UrlTestRuntime, DEFAULT_PROBE_URL};
