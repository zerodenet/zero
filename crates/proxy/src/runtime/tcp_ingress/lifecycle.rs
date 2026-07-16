//! TCP ingress lifecycle facade.
//!
//! The root stays as a facade so session setup, relay outcome handling, and
//! rate-limit projection do not regrow into one implementation bucket.

mod rate_limit;
mod result;
mod serve;

pub(crate) use rate_limit::apply_kernel_rate_limits_from_config;
pub(crate) use serve::serve_inbound;
#[cfg(any(feature = "socks5", feature = "hysteria2", feature = "mieru"))]
pub(crate) use serve::serve_inbound_with_client_response;
