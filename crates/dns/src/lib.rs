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
///
/// Inner state is under a read-write lock so DNS config can be
/// hot-reloaded without restarting the proxy.
pub struct DnsSystem {
    inner: std::sync::RwLock<DnsSystemInner>,
}

impl fmt::Debug for DnsSystem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &*self.inner.read().expect("dns system lock poisoned") {
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

/// Snapshot of the fields needed for an async `resolve()` call.
/// Extracted from the lock so we don't hold it across await points.
struct ResolveSnapshot {
    servers: Vec<Arc<ResolverBackend>>,
    router: DnsRouter,
    cache: Option<DnsCache>,
    fake_ip: Option<Arc<FakeIpAllocator>>,
}

impl DnsSystem {
    /// Build a `DnsSystem` from optional config.
    pub fn build(config: Option<&DnsConfig>) -> io::Result<Self> {
        Ok(Self {
            inner: std::sync::RwLock::new(Self::build_inner(config)?),
        })
    }

    fn build_inner(config: Option<&DnsConfig>) -> io::Result<DnsSystemInner> {
        let Some(cfg) = config else {
            return Ok(DnsSystemInner::System(TokioSystemResolver));
        };

        if cfg.servers.is_empty() {
            return Ok(DnsSystemInner::System(TokioSystemResolver));
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
            .map(FakeIpAllocator::new)
            .transpose()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?
            .map(Arc::new);

        Ok(DnsSystemInner::Configured {
            servers,
            router,
            cache,
            fake_ip,
        })
    }

    /// Hot-reload DNS configuration.
    ///
    /// In-flight resolutions continue using the old inner state until they
    /// complete; new resolutions see the updated config immediately.
    pub fn reload(&self, config: Option<&DnsConfig>) -> io::Result<()> {
        let new_inner = Self::build_inner(config)?;
        let mut guard = self.inner.write().expect("dns system lock poisoned");
        *guard = new_inner;
        Ok(())
    }

    /// Reverse lookup: fake IP → real domain.
    /// Used before route_decision to restore the original target domain.
    pub async fn lookup_fake_ip(&self, ip: &IpAddress) -> Option<String> {
        let fake_ip = {
            let guard = self.inner.read().expect("dns system lock poisoned");
            match &*guard {
                DnsSystemInner::Configured {
                    fake_ip: Some(alloc),
                    ..
                } => Some(Arc::clone(alloc)),
                _ => None,
            }
        };
        match fake_ip {
            Some(alloc) => alloc.lookup(ip).await,
            None => None,
        }
    }

    /// Take a snapshot of the current inner state for an async resolve.
    fn snapshot(&self) -> Option<ResolveSnapshot> {
        let guard = self.inner.read().expect("dns system lock poisoned");
        match &*guard {
            DnsSystemInner::System(_) => None,
            DnsSystemInner::Configured {
                servers,
                router,
                cache,
                fake_ip,
            } => Some(ResolveSnapshot {
                servers: servers.clone(),
                router: router.clone(),
                cache: cache.clone(),
                fake_ip: fake_ip.clone(),
            }),
        }
    }
}

impl DnsResolver for DnsSystem {
    type Error = io::Error;

    async fn resolve(&self, domain: &str) -> Result<Vec<IpAddress>, Self::Error> {
        let snapshot = match self.snapshot() {
            Some(s) => s,
            None => {
                // System resolver fallback — extract the resolver from
                // the lock, drop the guard, then await.
                let sys_resolver = {
                    let guard = self.inner.read().expect("dns system lock poisoned");
                    match &*guard {
                        DnsSystemInner::System(r) => *r,
                        _ => TokioSystemResolver,
                    }
                };
                return sys_resolver.resolve(domain).await;
            }
        };

        // Fake IP path: return synthetic IP instead of real resolution.
        if let Some(alloc) = &snapshot.fake_ip {
            if !alloc.is_excluded(domain) {
                if let Some(ip) = alloc.alloc(domain).await {
                    return Ok(vec![ip]);
                }
            }
        }

        // 1. Check cache.
        if let Some(ref c) = snapshot.cache {
            if let Some(ips) = c.get(domain).await {
                return Ok(ips);
            }
        }

        // 2. Route → primary index, reorder so primary is first.
        let primary = snapshot.router.route(domain);
        let ordered = {
            let n = snapshot.servers.len();
            let mut v: Vec<Arc<ResolverBackend>> = Vec::with_capacity(n);
            if primary < n {
                v.push(Arc::clone(&snapshot.servers[primary]));
            }
            for i in 0..n {
                if i != primary {
                    v.push(Arc::clone(&snapshot.servers[i]));
                }
            }
            v
        };

        // 3. Race all backends concurrently, take first success.
        let result = race_resolve(domain, &ordered).await;

        // 4. Cache on success (default TTL 300s).
        if let (Some(c), Ok(ref ips)) = (&snapshot.cache, &result) {
            c.put(domain.to_owned(), ips.clone(), 300).await;
        }

        result
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

    let mut last_err = io::Error::other("no dns backends configured");
    while let Some(result) = tasks.join_next().await {
        match result {
            Ok(Ok(ips)) => {
                tasks.abort_all();
                return Ok(ips);
            }
            Ok(Err(e)) => last_err = e,
            Err(join_err) => {
                last_err = io::Error::other(join_err.to_string());
            }
        }
    }

    Err(last_err)
}
