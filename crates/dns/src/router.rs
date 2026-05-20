//! Domain-based DNS server routing.

use zero_config::DnsRouteConfig;

/// A compiled DNS route rule.
enum DnsRoutePattern {
    /// Exact domain match, e.g. `"example.com"`.
    Exact(String),
    /// Suffix match, e.g. `".example.com"` matches `*.example.com`.
    Suffix(String),
}

impl DnsRoutePattern {
    fn matches(&self, domain: &str) -> bool {
        let domain = domain.to_ascii_lowercase();
        match self {
            Self::Exact(pattern) => pattern.as_str() == domain,
            Self::Suffix(pattern) => domain == &pattern[1..] || domain.ends_with(pattern.as_str()),
        }
    }
}

pub(crate) struct DnsRouter {
    /// Ordered rules: (pattern, server_index). First match wins.
    rules: Vec<(DnsRoutePattern, usize)>,
    /// Default server index when no rule matches.
    default_idx: usize,
}

impl DnsRouter {
    /// Create a router from config routes.
    ///
    /// `num_servers` is the total number of configured DNS servers.
    pub(crate) fn new(routes: &[DnsRouteConfig], _num_servers: usize) -> Self {
        let rules: Vec<_> = routes
            .iter()
            .filter_map(|r| {
                let idx = if r.server == "system" {
                    0
                } else {
                    r.server.parse::<usize>().ok()?
                };
                let pattern = if let Some(suffix) = r.domain.strip_prefix("*") {
                    DnsRoutePattern::Suffix(suffix.to_ascii_lowercase())
                } else {
                    DnsRoutePattern::Exact(r.domain.to_ascii_lowercase())
                };
                Some((pattern, idx))
            })
            .collect();

        Self {
            rules,
            default_idx: 0,
        }
    }

    /// Return the server index for a domain. First match wins.
    pub(crate) fn route(&self, domain: &str) -> usize {
        for (pattern, idx) in &self.rules {
            if pattern.matches(domain) {
                return *idx;
            }
        }
        self.default_idx
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_match() {
        let routes = vec![DnsRouteConfig {
            domain: "example.com".into(),
            server: "1".into(),
        }];
        let router = DnsRouter::new(&routes, 2);
        assert_eq!(router.route("example.com"), 1);
        assert_eq!(router.route("other.com"), 0);
    }

    #[test]
    fn wildcard_match() {
        let routes = vec![DnsRouteConfig {
            domain: "*.google.com".into(),
            server: "1".into(),
        }];
        let router = DnsRouter::new(&routes, 2);
        assert_eq!(router.route("www.google.com"), 1);
        assert_eq!(router.route("mail.google.com"), 1);
        assert_eq!(router.route("google.com"), 1); // suffix ".google.com" matches base
        assert_eq!(router.route("other.com"), 0);
    }

    #[test]
    fn first_match_wins() {
        let routes = vec![
            DnsRouteConfig {
                domain: "*.internal.example.com".into(),
                server: "1".into(),
            },
            DnsRouteConfig {
                domain: "*.example.com".into(),
                server: "2".into(),
            },
        ];
        let router = DnsRouter::new(&routes, 3);
        assert_eq!(router.route("app.internal.example.com"), 1);
        assert_eq!(router.route("app.example.com"), 2);
    }
}
