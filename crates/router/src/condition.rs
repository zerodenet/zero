use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::Arc;

use ipnet::IpNet;
use zero_core::Address;
use zero_rule::PreparedRuleQuery;

use crate::{RouteContext, RuleSetMatcher};

/// Wrapper around compiled regex -- compares by original pattern string.
#[derive(Clone)]
pub struct CompiledRegex {
    pattern: String,
    re: Arc<regex::Regex>,
}

impl std::fmt::Debug for CompiledRegex {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CompiledRegex")
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

    /// The original pattern string this regex was compiled from.
    pub fn pattern(&self) -> &str {
        &self.pattern
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleCondition {
    Inbound(Vec<String>),
    Domain(Vec<String>),
    DomainKeyword(Vec<String>),
    DomainRegex(Vec<CompiledRegex>),
    Ip(Vec<IpNet>),
    RuleSet(RuleSetMatcher),
    GeoIp(Vec<String>),
    Sni(Vec<String>),
    And(Vec<RuleCondition>),
    Or(Vec<RuleCondition>),
}

/// Human-readable summary of a [`RuleCondition`], for diagnostics/trace.
pub fn condition_describe(condition: &RuleCondition) -> String {
    let join = |values: &[String]| values.join(", ");
    match condition {
        RuleCondition::Inbound(values) => format!("inbound: {}", join(values)),
        RuleCondition::Domain(values) => format!("domain: {}", join(values)),
        RuleCondition::DomainKeyword(values) => format!("domain_keyword: {}", join(values)),
        RuleCondition::DomainRegex(values) => {
            let patterns: Vec<&str> = values.iter().map(CompiledRegex::pattern).collect();
            format!("domain_regex: {}", patterns.join(", "))
        }
        RuleCondition::Ip(values) => {
            let networks: Vec<String> = values.iter().map(ToString::to_string).collect();
            format!("ip: {}", networks.join(", "))
        }
        RuleCondition::RuleSet(rule_set) => format!("rule_set: {}", rule_set.tag()),
        RuleCondition::GeoIp(values) => format!("geoip: {}", join(values)),
        RuleCondition::Sni(values) => format!("sni: {}", join(values)),
        RuleCondition::And(items) => {
            let inner: Vec<String> = items.iter().map(condition_describe).collect();
            format!("and({})", inner.join(", "))
        }
        RuleCondition::Or(items) => {
            let inner: Vec<String> = items.iter().map(condition_describe).collect();
            format!("or({})", inner.join(", "))
        }
    }
}

pub(crate) fn condition_matches(
    condition: &RuleCondition,
    context: RouteContext<'_>,
    geoip_db: Option<&maxminddb::Reader<Vec<u8>>>,
    rule_query: Option<&PreparedRuleQuery>,
) -> bool {
    match condition {
        RuleCondition::Inbound(tags) => context
            .inbound_tag
            .map(|inbound_tag| tags.iter().any(|tag| tag == inbound_tag))
            .unwrap_or(false),
        RuleCondition::Domain(patterns) => match context.address {
            Address::Domain(domain) => patterns
                .iter()
                .any(|pattern| domain_matches(pattern, domain)),
            _ => false,
        },
        RuleCondition::DomainKeyword(keywords) => match context.address {
            Address::Domain(domain) => keywords.iter().any(|keyword| {
                domain
                    .to_ascii_lowercase()
                    .contains(&keyword.to_ascii_lowercase())
            }),
            _ => false,
        },
        RuleCondition::DomainRegex(patterns) => match context.address {
            Address::Domain(domain) => patterns.iter().any(|regex| regex.re.is_match(domain)),
            _ => false,
        },
        RuleCondition::Ip(networks) => address_to_ip(context.address)
            .map(|address| networks.iter().any(|network| network.contains(&address)))
            .unwrap_or(false),
        RuleCondition::RuleSet(rule_set) => rule_query
            .map(|query| rule_set.matches(query))
            .unwrap_or(false),
        RuleCondition::GeoIp(codes) => match (address_to_ip(context.address), geoip_db) {
            (Some(address), Some(database)) => database
                .lookup::<maxminddb::geoip2::Country>(address)
                .ok()
                .and_then(|country| country.country)
                .and_then(|country| country.iso_code)
                .map(|code| {
                    codes
                        .iter()
                        .any(|candidate| candidate.eq_ignore_ascii_case(code))
                })
                .unwrap_or(false),
            _ => false,
        },
        RuleCondition::Sni(patterns) => context
            .sni
            .map(|sni| patterns.iter().any(|pattern| domain_matches(pattern, sni)))
            .unwrap_or(false),
        RuleCondition::And(conditions) => conditions
            .iter()
            .all(|item| condition_matches(item, context, geoip_db, rule_query)),
        RuleCondition::Or(conditions) => conditions
            .iter()
            .any(|item| condition_matches(item, context, geoip_db, rule_query)),
    }
}

pub(crate) fn prepare_rule_query(address: &Address) -> Option<PreparedRuleQuery> {
    match address {
        Address::Domain(domain) => PreparedRuleQuery::new(Some(domain), None).ok(),
        Address::Ipv4(bytes) => {
            PreparedRuleQuery::new(None, Some(IpAddr::V4(Ipv4Addr::from(*bytes)))).ok()
        }
        Address::Ipv6(bytes) => {
            PreparedRuleQuery::new(None, Some(IpAddr::V6(Ipv6Addr::from(*bytes)))).ok()
        }
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
