//! TTL-based DNS cache with simple eviction.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;
use zero_config::DnsCacheConfig;
use zero_traits::IpAddress;

struct CacheEntry {
    ips: Vec<IpAddress>,
    expires_at: Instant,
}

pub(crate) struct DnsCache {
    map: Mutex<HashMap<String, CacheEntry>>,
    max_entries: usize,
    max_ttl: Option<Duration>,
}

impl DnsCache {
    pub(crate) fn new(config: &DnsCacheConfig) -> Self {
        Self {
            map: Mutex::new(HashMap::new()),
            max_entries: config.max_entries,
            max_ttl: config.max_ttl_seconds.map(Duration::from_secs),
        }
    }

    /// Look up domain in cache. Returns `None` on miss or expiry.
    pub(crate) async fn get(&self, domain: &str) -> Option<Vec<IpAddress>> {
        let map = self.map.lock().await;
        let entry = map.get(domain)?;
        if entry.expires_at <= Instant::now() {
            return None;
        }
        Some(entry.ips.clone())
    }

    /// Store a resolution in the cache.
    pub(crate) async fn put(&self, domain: String, ips: Vec<IpAddress>, ttl_seconds: u64) {
        let effective_ttl = self
            .max_ttl
            .map(|max| max.min(Duration::from_secs(ttl_seconds)))
            .unwrap_or(Duration::from_secs(ttl_seconds));

        let mut map = self.map.lock().await;
        if map.len() >= self.max_entries {
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
        if map.len() >= self.max_entries {
            // Simple: remove the oldest half by insertion order approximation.
            let to_remove = map.len() / 2;
            let keys: Vec<String> = map.keys().take(to_remove).cloned().collect();
            for k in keys {
                map.remove(&k);
            }
        }
    }

}
