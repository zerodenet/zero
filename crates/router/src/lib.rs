use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::Arc;

use ipnet::IpNet;
use zero_core::Address;

/// Wrapper around compiled regex — compares by original pattern string.
#[derive(Clone)]
pub struct CompiledRegex {
    pattern: String,
    re: Arc<regex::Regex>,
}

impl std::fmt::Debug for CompiledRegex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompiledRegex")
            .field("pattern", &self.pattern)
            .finish()
    }
}

impl PartialEq for CompiledRegex {
    fn eq(&self, other: &Self) -> bool {
        self.pattern == other.pattern
    }
}

impl Eq for CompiledRegex {}

impl CompiledRegex {
    pub fn new(pattern: String) -> Result<Self, regex::Error> {
        Ok(Self {
            re: Arc::new(regex::Regex::new(&pattern)?),
            pattern,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleCondition {
    Domain(Vec<String>),
    DomainKeyword(Vec<String>),
    DomainRegex(Vec<CompiledRegex>),
    Ip(Vec<IpNet>),
    GeoIp(Vec<String>),
    Sni(Vec<String>),
    And(Vec<RuleCondition>),
    Or(Vec<RuleCondition>),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum RouteAction {
    Route(String),
    #[default]
    Direct,
    Reject,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    pub condition: RuleCondition,
    pub action: RouteAction,
}

pub struct RuleSet {
    pub rules: Vec<Rule>,
    pub final_action: RouteAction,
    pub geoip_db: Option<Arc<maxminddb::Reader<Vec<u8>>>>,
}

impl std::fmt::Debug for RuleSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RuleSet")
            .field("rules", &self.rules)
            .field("final_action", &self.final_action)
            .field("geoip_db", &self.geoip_db.is_some())
            .finish()
    }
}

impl RuleSet {
    pub fn new(rules: Vec<Rule>, final_action: RouteAction) -> Self {
        Self {
            rules,
            final_action,
            geoip_db: None,
        }
    }

    pub fn with_geoip(
        rules: Vec<Rule>,
        final_action: RouteAction,
        db: Arc<maxminddb::Reader<Vec<u8>>>,
    ) -> Self {
        Self {
            rules,
            final_action,
            geoip_db: Some(db),
        }
    }

    pub fn decide_ref(&self, address: &Address, sni: Option<&str>) -> &RouteAction {
        self.rules
            .iter()
            .find(|rule| condition_matches(&rule.condition, address, sni, self.geoip_db.as_deref()))
            .map(|rule| &rule.action)
            .unwrap_or(&self.final_action)
    }

    pub fn decide(&self, address: &Address, sni: Option<&str>) -> RouteAction {
        self.decide_ref(address, sni).clone()
    }
}

fn condition_matches(
    condition: &RuleCondition,
    address: &Address,
    sni: Option<&str>,
    geoip_db: Option<&maxminddb::Reader<Vec<u8>>>,
) -> bool {
    match condition {
        RuleCondition::Domain(patterns) => match address {
            Address::Domain(domain) => patterns
                .iter()
                .any(|pattern| domain_matches(pattern, domain)),
            _ => false,
        },
        RuleCondition::DomainKeyword(keywords) => match address {
            Address::Domain(domain) => keywords.iter().any(|kw| {
                domain
                    .to_ascii_lowercase()
                    .contains(&kw.to_ascii_lowercase())
            }),
            _ => false,
        },
        RuleCondition::DomainRegex(patterns) => match address {
            Address::Domain(domain) => patterns.iter().any(|re| re.re.is_match(domain)),
            _ => false,
        },
        RuleCondition::Ip(networks) => match address_to_ip(address) {
            Some(ip) => networks.iter().any(|network| network.contains(&ip)),
            None => false,
        },
        RuleCondition::GeoIp(codes) => match (address_to_ip(address), geoip_db) {
            (Some(ip), Some(db)) => {
                if let Ok(country) = db.lookup::<maxminddb::geoip2::Country>(ip) {
                    country
                        .country
                        .and_then(|c| c.iso_code)
                        .map(|cc| codes.iter().any(|code| code.eq_ignore_ascii_case(cc)))
                        .unwrap_or(false)
                } else {
                    false
                }
            }
            _ => false,
        },
        RuleCondition::Sni(patterns) => match sni {
            Some(sni) => patterns.iter().any(|pattern| domain_matches(pattern, sni)),
            None => false,
        },
        RuleCondition::And(conditions) => conditions
            .iter()
            .all(|c| condition_matches(c, address, sni, geoip_db)),
        RuleCondition::Or(conditions) => conditions
            .iter()
            .any(|c| condition_matches(c, address, sni, geoip_db)),
    }
}

fn domain_matches(pattern: &str, domain: &str) -> bool {
    let pattern = pattern.trim().trim_start_matches('.').to_ascii_lowercase();
    let domain = domain.to_ascii_lowercase();

    domain == pattern || domain.ends_with(&format!(".{pattern}"))
}

fn address_to_ip(address: &Address) -> Option<IpAddr> {
    match address {
        Address::Domain(_) => None,
        Address::Ipv4(bytes) => Some(IpAddr::V4(Ipv4Addr::from(*bytes))),
        Address::Ipv6(bytes) => Some(IpAddr::V6(Ipv6Addr::from(*bytes))),
    }
}
