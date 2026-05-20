//! Fake IP allocator — maps domains to synthetic IPs for transparent proxying.
//!
//! When a client queries `google.com`, we return `198.18.0.5` instead
//! of the real IP. When the client later connects to `198.18.0.5:443`,
//! we look up `google.com` from the reverse map and route based on the
//! real domain name.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;
use zero_config::FakeIpConfig;
use zero_traits::IpAddress;

/// Allocates fake IPs from a configurable CIDR pool.
pub struct FakeIpAllocator {
    inner: Mutex<AllocatorInner>,
    ttl: Duration,
    exclude_domains: Vec<String>,
}

struct AllocatorInner {
    /// The next IP to try allocating (linear scan from network base).
    next_ip: u32,
    /// Network base as u32.
    base: u32,
    /// Subnet mask.
    mask: u32,
    /// Domain → assigned fake IP (IpAddress::V4).
    forward: HashMap<String, IpAddress>,
    /// Fake IP bytes → domain.
    reverse: HashMap<[u8; 4], (String, Instant)>,
}

impl FakeIpAllocator {
    /// Parse the CIDR and build the allocator.
    pub fn new(config: &FakeIpConfig) -> Result<Self, String> {
        let net: ipnet::IpNet = config.cidr.parse().map_err(|e| format!("invalid cidr: {e}"))?;
        let base = match net.addr() {
            std::net::IpAddr::V4(v4) => u32::from_be_bytes(v4.octets()),
            std::net::IpAddr::V6(_) => return Err("fake ip only supports IPv4 CIDR".into()),
        };
        let mask = match net.netmask() {
            std::net::IpAddr::V4(v4) => u32::from_be_bytes(v4.octets()),
            std::net::IpAddr::V6(_) => unreachable!(),
        };

        Ok(Self {
            inner: Mutex::new(AllocatorInner {
                next_ip: base + 1, // skip network address
                base,
                mask,
                forward: HashMap::new(),
                reverse: HashMap::new(),
            }),
            ttl: Duration::from_secs(config.ttl_seconds),
            exclude_domains: config.exclude_domains.clone(),
        })
    }

    /// Check if a domain should skip fake IP.
    pub fn is_excluded(&self, domain: &str) -> bool {
        let domain = domain.to_ascii_lowercase();
        self.exclude_domains.iter().any(|pattern| {
            if let Some(suffix) = pattern.strip_prefix('*') {
                domain.ends_with(suffix)
            } else {
                pattern.as_str() == domain
            }
        })
    }

    /// Allocate a fake IP for a domain, or return the existing one.
    /// Returns `None` if the pool is exhausted.
    pub async fn alloc(&self, domain: &str) -> Option<IpAddress> {
        let mut inner = self.inner.lock().await;

        // Check existing — copy IP value before mutable borrow on reverse.
        if let Some(ip) = inner.forward.get(domain) {
            let existing = *ip;
            // Refresh TTL.
            if let IpAddress::V4(octets) = existing {
                if let Some(entry) = inner.reverse.get_mut(&octets) {
                    entry.1 = Instant::now() + self.ttl;
                }
            }
            return Some(existing);
        }

        // Allocate new.
        let broadcast = inner.base | !inner.mask;
        let start = inner.next_ip;
        let mut ip = start;
        loop {
            let octets = u32::to_be_bytes(ip);
            // Don't use network address, broadcast, or already-assigned but expired IPs.
            if ip != inner.base && ip != broadcast {
                match inner.reverse.get(&octets) {
                    None => {
                        // Free — use it.
                        inner.forward.insert(domain.to_owned(), IpAddress::V4(octets));
                        inner.reverse.insert(octets, (domain.to_owned(), Instant::now() + self.ttl));
                        inner.next_ip = if ip + 1 > broadcast - 1 { inner.base + 1 } else { ip + 1 };
                        return Some(IpAddress::V4(octets));
                    }
                    Some((_, expires)) if *expires <= Instant::now() => {
                        // Expired — reclaim.
                        let old_domain = inner.reverse.remove(&octets).unwrap().0;
                        inner.forward.remove(&old_domain);
                        inner.forward.insert(domain.to_owned(), IpAddress::V4(octets));
                        inner.reverse.insert(octets, (domain.to_owned(), Instant::now() + self.ttl));
                        inner.next_ip = if ip + 1 > broadcast - 1 { inner.base + 1 } else { ip + 1 };
                        return Some(IpAddress::V4(octets));
                    }
                    Some(_) => { /* in use, skip */ }
                }
            }
            ip += 1;
            if ip > broadcast - 1 {
                ip = inner.base + 1;
            }
            if ip == start {
                return None; // pool exhausted
            }
        }
    }

    /// Reverse lookup: fake IP → domain.
    pub async fn lookup(&self, ip: &IpAddress) -> Option<String> {
        let octets = match ip {
            IpAddress::V4(o) => *o,
            _ => return None,
        };
        let inner = self.inner.lock().await;
        inner.reverse.get(&octets).map(|(d, _)| d.clone())
    }

    /// Evict expired entries. Call periodically or on allocation.
    #[allow(dead_code)]
    pub async fn evict_expired(&self) {
        let mut inner = self.inner.lock().await;
        let now = Instant::now();
        let expired: Vec<[u8; 4]> = inner
            .reverse
            .iter()
            .filter(|(_, (_, expires))| *expires <= now)
            .map(|(octets, _)| *octets)
            .collect();
        for octets in expired {
            if let Some((domain, _)) = inner.reverse.remove(&octets) {
                inner.forward.remove(&domain);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> FakeIpConfig {
        FakeIpConfig {
            cidr: "198.18.0.0/24".into(),
            ttl_seconds: 3600,
            exclude_domains: vec![],
        }
    }

    #[tokio::test]
    async fn alloc_and_lookup() {
        let alloc = FakeIpAllocator::new(&test_config()).unwrap();
        let ip = alloc.alloc("google.com").await.unwrap();
        assert_eq!(alloc.lookup(&ip).await.unwrap(), "google.com");
    }

    #[tokio::test]
    async fn same_domain_same_ip() {
        let alloc = FakeIpAllocator::new(&test_config()).unwrap();
        let ip1 = alloc.alloc("google.com").await.unwrap();
        let ip2 = alloc.alloc("google.com").await.unwrap();
        assert_eq!(ip1, ip2);
    }

    #[tokio::test]
    async fn different_domains_different_ips() {
        let alloc = FakeIpAllocator::new(&test_config()).unwrap();
        let ip1 = alloc.alloc("google.com").await.unwrap();
        let ip2 = alloc.alloc("github.com").await.unwrap();
        assert_ne!(ip1, ip2);
    }

    #[tokio::test]
    async fn excluded_domain() {
        let mut cfg = test_config();
        cfg.exclude_domains = vec!["*.local".into(), "example.com".into()];
        let alloc = FakeIpAllocator::new(&cfg).unwrap();
        assert!(alloc.is_excluded("app.local"));
        assert!(alloc.is_excluded("example.com"));
        assert!(!alloc.is_excluded("google.com"));
    }
}
