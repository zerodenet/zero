use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DnsConfig {
    /// Ordered DNS servers. Resolution races all servers concurrently and
    /// returns the first success. Empty or omitted defaults to the system resolver.
    #[serde(default)]
    pub servers: Vec<DnsServerConfig>,

    /// Optional TTL-based DNS cache.
    #[serde(default)]
    pub cache: Option<DnsCacheConfig>,

    /// Per-domain routing rules. First match wins; unmatched domains use
    /// the first server in the servers list.
    #[serde(default)]
    pub routes: Vec<DnsRouteConfig>,

    /// Fake IP mode: return synthetic IPs to clients and maintain
    /// a domain↔IP mapping for transparent proxying.
    #[serde(default)]
    pub fake_ip: Option<FakeIpConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FakeIpConfig {
    /// CIDR block for the fake IP pool, e.g. `"198.18.0.0/15"`.
    pub cidr: String,
    /// How long a fake IP assignment lasts before expiry (seconds).
    #[serde(default = "default_fake_ip_ttl")]
    pub ttl_seconds: u64,
    /// Domains excluded from fake IP (always use real DNS).
    #[serde(default)]
    pub exclude_domains: Vec<String>,
}

const fn default_fake_ip_ttl() -> u64 {
    86400
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum DnsServerConfig {
    /// System resolver (getaddrinfo).
    #[serde(rename = "system")]
    System,
    /// Plain UDP DNS.
    #[serde(rename = "udp")]
    Udp {
        address: String,
        #[serde(default = "default_dns_port")]
        port: u16,
    },
    /// DNS-over-HTTPS (v2).
    #[serde(rename = "doh")]
    Doh {
        url: String,
        #[serde(default)]
        server_name: Option<String>,
    },
    /// DNS-over-TLS (v2).
    #[serde(rename = "dot")]
    Dot {
        address: String,
        #[serde(default = "default_dns_dot_port")]
        port: u16,
        #[serde(default)]
        server_name: Option<String>,
    },
}

const fn default_dns_port() -> u16 {
    53
}
const fn default_dns_dot_port() -> u16 {
    853
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DnsCacheConfig {
    /// Max cached domain entries. Default 256.
    #[serde(default = "default_dns_cache_max_entries")]
    pub max_entries: usize,
    /// Cap TTL at this value (seconds). Omit to use DNS record TTL.
    #[serde(default)]
    pub max_ttl_seconds: Option<u64>,
}

const fn default_dns_cache_max_entries() -> usize {
    256
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DnsRouteConfig {
    /// Domain pattern. Exact ("example.com") or wildcard ("*.example.com").
    pub domain: String,
    /// Server identifier. Either `"system"` or a 0-based index into the
    /// `servers` array as a string (e.g. `"0"`, `"1"`).
    pub server: String,
}
