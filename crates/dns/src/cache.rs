//! TTL-based DNS cache with simple eviction.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;
use zero_config::DnsCacheConfig;
use zero_traits::IpAddress;

struct CacheEntry {
    ips: Vec<IpAddress>,
    expires_at: Instant,
}

pub(crate) struct DnsCache {
    inner: Arc<DnsCacheInner>,
}

struct DnsCacheInner {
    map: Mutex<HashMap<String, CacheEntry>>,
    max_entries: usize,
    max_ttl: Option<Duration>,
}

impl Clone for DnsCache {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl DnsCache {
    pub(crate) fn new(config: &DnsCacheConfig) -> Self {
        Self {
            inner: Arc::new(DnsCacheInner {
                map: Mutex::new(HashMap::new()),
                max_entries: config.max_entries,
                max_ttl: config.max_ttl_seconds.map(Duration::from_secs),
            }),
        }
    }

    /// Look up domain in cache. Returns `None` on miss or expiry.
    pub(crate) async fn get(&self, domain: &str) -> Option<Vec<IpAddress>> {
        let map = self.inner.map.lock().await;
        let entry = map.get(domain)?;
        if entry.expires_at <= Instant::now() {
            return None;
        }
        Some(entry.ips.clone())
    }

    /// Inspect a single cached entry (diagnostic). Returns the addresses and
    /// seconds until expiry. `None` on miss or expiry.
    pub(crate) async fn inspect(&self, domain: &str) -> Option<(Vec<IpAddress>, u64)> {
        let map = self.inner.map.lock().await;
        let entry = map.get(domain)?;
        let now = Instant::now();
        if entry.expires_at <= now {
            return None;
        }
        Some((
            entry.ips.clone(),
            entry.expires_at.duration_since(now).as_secs(),
        ))
    }

    /// Snapshot all live cache entries (diagnostic), capped to `limit`.
    /// Returns `(domain, addresses, seconds until expiry)` per entry.
    pub(crate) async fn entries(&self, limit: usize) -> Vec<(String, Vec<IpAddress>, u64)> {
        let map = self.inner.map.lock().await;
        let now = Instant::now();
        map.iter()
            .filter(|(_, entry)| entry.expires_at > now)
            .take(limit)
            .map(|(domain, entry)| {
                (
                    domain.clone(),
                    entry.ips.clone(),
                    entry.expires_at.duration_since(now).as_secs(),
                )
            })
            .collect()
    }

    /// Store a resolution in the cache.
    pub(crate) async fn put(&self, domain: String, ips: Vec<IpAddress>, ttl_seconds: u64) {
        let effective_ttl = self
            .inner
            .max_ttl
            .map(|max| max.min(Duration::from_secs(ttl_seconds)))
            .unwrap_or(Duration::from_secs(ttl_seconds));

        let mut map = self.inner.map.lock().await;
        if map.len() >= self.inner.max_entries {
            self.evict(&mut map);
        }
        map.insert(
            domain,
            CacheEntry {
                ips,
                expires_at: Instant::now() + effective_ttl,
            },
        );
    }

    /// Evict expired entries. If still over capacity, clear the oldest half.
    fn evict(&self, map: &mut HashMap<String, CacheEntry>) {
        let now = Instant::now();
        map.retain(|_, v| v.expires_at > now);
        if map.len() >= self.inner.max_entries {
            // Simple: remove the oldest half by insertion order approximation.
            let to_remove = map.len() / 2;
            let keys: Vec<String> = map.keys().take(to_remove).cloned().collect();
            for k in keys {
                map.remove(&k);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zero_config::DnsCacheConfig;

    fn cfg() -> DnsCacheConfig {
        DnsCacheConfig {
            max_entries: 16,
            max_ttl_seconds: None,
        }
    }

    #[tokio::test]
    async fn inspect_returns_addresses_and_ttl() {
        let cache = DnsCache::new(&cfg());
        cache
            .put(
                "example.com".into(),
                vec![IpAddress::V4([93, 184, 216, 34])],
                300,
            )
            .await;
        let (ips, ttl) = cache.inspect("example.com").await.expect("cached entry");
        assert_eq!(ips.len(), 1);
        assert!(ttl <= 300, "ttl should be capped at the stored value");
        assert!(cache.inspect("missing.com").await.is_none());
    }

    #[tokio::test]
    async fn entries_lists_live_entries_capped() {
        let cache = DnsCache::new(&cfg());
        cache
            .put("a.com".into(), vec![IpAddress::V4([1, 1, 1, 1])], 300)
            .await;
        cache
            .put("b.com".into(), vec![IpAddress::V4([2, 2, 2, 2])], 300)
            .await;
        let entries = cache.entries(16).await;
        assert_eq!(entries.len(), 2);
        let capped = cache.entries(1).await;
        assert_eq!(capped.len(), 1);
    }
}
