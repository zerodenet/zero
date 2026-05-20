//! System resolver backed by `tokio::net::lookup_host`.

use std::io;
use std::net::IpAddr;

use tokio::net::lookup_host;
use zero_traits::{DnsResolver, IpAddress};

/// Thin wrapper around tokio's `lookup_host` — the OS resolver.
#[derive(Debug, Clone, Copy)]
pub(crate) struct TokioSystemResolver;

impl DnsResolver for TokioSystemResolver {
    type Error = io::Error;

    async fn resolve(&self, domain: &str) -> Result<Vec<IpAddress>, Self::Error> {
        let mut resolved = Vec::new();
        for addr in lookup_host((domain, 0)).await? {
            resolved.push(ip_addr_to_ip(addr.ip()));
        }
        Ok(resolved)
    }
}

fn ip_addr_to_ip(addr: IpAddr) -> IpAddress {
    match addr {
        IpAddr::V4(v4) => IpAddress::V4(v4.octets()),
        IpAddr::V6(v6) => IpAddress::V6(v6.octets()),
    }
}
