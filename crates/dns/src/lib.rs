//! DNS subsystem — configurable resolver, caching, and routing.
//!
//! When no DNS configuration is provided, `DnsSystem` degrades to the
//! system resolver via `TokioResolver`, preserving existing behavior
//! with zero additional allocation.

mod backends;
mod cache;
mod fake_ip;
mod router;
mod system;
pub mod udp; // DNS wire helpers (build_dns_response, etc.) always available

use std::fmt;
use std::io;
use std::sync::Arc;

use zero_config::DnsConfig;
use zero_traits::{DnsResolver, IpAddress};

use backends::ResolverBackend;
use cache::DnsCache;
use fake_ip::FakeIpAllocator;
use router::DnsRouter;
use system::TokioSystemResolver;

/// The configured DNS subsystem.
///
/// Implements [`DnsResolver`] so it can be passed directly to
/// `DirectConnector` and all upstream handlers.
pub struct DnsSystem {
    inner: DnsSystemInner,
}

impl fmt::Debug for DnsSystem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.inner {
            DnsSystemInner::System(_) => f.debug_tuple("System").finish(),
            DnsSystemInner::Configured { servers, .. } => f
                .debug_struct("Configured")
                .field("servers", &servers.len())
                .finish(),
        }
    }
}

enum DnsSystemInner {
    /// No DNS config supplied — passthrough to the system resolver.
    System(TokioSystemResolver),
    /// Fully configured with servers, routing, cache, and optional fake IP.
    Configured {
        servers: Vec<Arc<ResolverBackend>>,
        router: DnsRouter,
        cache: Option<DnsCache>,
        fake_ip: Option<Arc<FakeIpAllocator>>,
    },
}

impl DnsSystem {
    /// Build a `DnsSystem` from optional config.
    pub fn build(config: Option<&DnsConfig>) -> io::Result<Self> {
        let Some(cfg) = config else {
            return Ok(Self {
                inner: DnsSystemInner::System(TokioSystemResolver),
            });
        };

        if cfg.servers.is_empty() {
            return Ok(Self {
                inner: DnsSystemInner::System(TokioSystemResolver),
            });
        }

        let mut servers: Vec<Arc<ResolverBackend>> = Vec::with_capacity(cfg.servers.len());
        for s in &cfg.servers {
            servers.push(Arc::new(ResolverBackend::build(s)?));
        }

        let router = DnsRouter::new(&cfg.routes, servers.len());
        let cache = cfg.cache.as_ref().map(DnsCache::new);
        let fake_ip = cfg
            .fake_ip
            .as_ref()
            .map(|c| FakeIpAllocator::new(c))
            .transpose()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?
            .map(Arc::new);

        Ok(Self {
            inner: DnsSystemInner::Configured {
                servers,
                router,
                cache,
                fake_ip,
            },
        })
    }

    /// Reverse lookup: fake IP → real domain.
    /// Used before route_decision to restore the original target domain.
    pub async fn lookup_fake_ip(&self, ip: &IpAddress) -> Option<String> {
        match &self.inner {
            DnsSystemInner::Configured {
                fake_ip: Some(alloc), ..
            } => alloc.lookup(ip).await,
            _ => None,
        }
    }
}

impl DnsResolver for DnsSystem {
    type Error = io::Error;

    async fn resolve(&self, domain: &str) -> Result<Vec<IpAddress>, Self::Error> {
        match &self.inner {
            DnsSystemInner::System(r) => r.resolve(domain).await,
            DnsSystemInner::Configured {
                servers,
                router,
                cache,
                fake_ip,
            } => {
                // Fake IP path: return synthetic IP instead of real resolution.
                if let Some(alloc) = fake_ip {
                    if !alloc.is_excluded(domain) {
                        if let Some(ip) = alloc.alloc(domain).await {
                            return Ok(vec![ip]);
                        }
                    }
                }

                // 1. Check cache.
                if let Some(c) = cache {
                    if let Some(ips) = c.get(domain).await {
                        return Ok(ips);
                    }
                }

                // 2. Route → primary index, reorder so primary is first.
                let primary = router.route(domain);
                let ordered = {
                    let n = servers.len();
                    let mut v: Vec<Arc<ResolverBackend>> = Vec::with_capacity(n);
                    if primary < n {
                        v.push(Arc::clone(&servers[primary]));
                    }
                    for i in 0..n {
                        if i != primary {
                            v.push(Arc::clone(&servers[i]));
                        }
                    }
                    v
                };

                // 3. Race all backends concurrently, take first success.
                let result = race_resolve(domain, &ordered).await;

                // 4. Cache on success (default TTL 300s).
                if let (Some(c), Ok(ref ips)) = (cache, &result) {
                    c.put(domain.to_owned(), ips.clone(), 300).await;
                }

                result
            }
        }
    }
}

/// Fire all backends concurrently via `JoinSet`, return first success.
async fn race_resolve(
    domain: &str,
    backends: &[Arc<ResolverBackend>],
) -> io::Result<Vec<IpAddress>> {
    use tokio::task::JoinSet;

    let mut tasks = JoinSet::new();
    for backend in backends {
        let b = Arc::clone(backend);
        let d = domain.to_owned();
        tasks.spawn(async move { b.resolve(&d).await });
    }

    let mut last_err = io::Error::new(io::ErrorKind::Other, "no dns backends configured");
    while let Some(result) = tasks.join_next().await {
        match result {
            Ok(Ok(ips)) => {
                tasks.abort_all();
                return Ok(ips);
            }
            Ok(Err(e)) => last_err = e,
            Err(join_err) => {
                last_err = io::Error::new(io::ErrorKind::Other, join_err.to_string());
            }
        }
    }

    Err(last_err)
}
