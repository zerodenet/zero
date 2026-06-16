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

    /// The original pattern string this regex was compiled from.
    pub fn pattern(&self) -> &str {
        &self.pattern
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

    /// Like [`decide`](Self::decide) but also returns which rule matched
    /// (index + condition summary), for `diagnostics.trace_route`.
    /// `matched_rule` is `None` when the decision came from `final_action`.
    pub fn decide_trace(&self, address: &Address, sni: Option<&str>) -> RouteDecision {
        if let Some((index, rule)) = self.rules.iter().enumerate().find(|(_, rule)| {
            condition_matches(&rule.condition, address, sni, self.geoip_db.as_deref())
        }) {
            RouteDecision {
                action: rule.action.clone(),
                matched_rule: Some(MatchedRule {
                    index,
                    condition: condition_describe(&rule.condition),
                }),
            }
        } else {
            RouteDecision {
                action: self.final_action.clone(),
                matched_rule: None,
            }
        }
    }
}

/// Human-readable summary of a [`RuleCondition`], for diagnostics/trace.
pub fn condition_describe(condition: &RuleCondition) -> String {
    let join = |vals: &[String]| vals.join(", ");
    match condition {
        RuleCondition::Domain(v) => format!("domain: {}", join(v)),
        RuleCondition::DomainKeyword(v) => format!("domain_keyword: {}", join(v)),
        RuleCondition::DomainRegex(v) => {
            let pats: Vec<&str> = v.iter().map(CompiledRegex::pattern).collect();
            format!("domain_regex: {}", pats.join(", "))
        }
        RuleCondition::Ip(v) => {
            let nets: Vec<String> = v.iter().map(|n| n.to_string()).collect();
            format!("ip: {}", nets.join(", "))
        }
        RuleCondition::GeoIp(v) => format!("geoip: {}", join(v)),
        RuleCondition::Sni(v) => format!("sni: {}", join(v)),
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

/// The rule that produced a routing decision (for `diagnostics.trace_route`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchedRule {
    /// 0-based index into `RuleSet::rules`.
    pub index: usize,
    /// Human-readable condition summary (see [`condition_describe`]).
    pub condition: String,
}

/// A routing decision plus the rule that produced it (if any).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteDecision {
    pub action: RouteAction,
    /// `None` when the decision came from `final_action` (no rule matched).
    pub matched_rule: Option<MatchedRule>,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn rs(rules: Vec<Rule>, final_action: RouteAction) -> RuleSet {
        RuleSet::new(rules, final_action)
    }

    #[test]
    fn decide_trace_reports_matched_rule_index_and_condition() {
        let rules = vec![
            Rule {
                condition: RuleCondition::Domain(vec!["example.com".to_owned()]),
                action: RouteAction::Reject,
            },
            Rule {
                condition: RuleCondition::Ip(vec!["10.0.0.0/8".parse().unwrap()]),
                action: RouteAction::Route("proxy".to_owned()),
            },
        ];
        let router = rs(rules, RouteAction::Direct);

        // First rule matches.
        let d = router.decide_trace(&Address::Domain("example.com".to_owned()), None);
        assert_eq!(d.action, RouteAction::Reject);
        let matched = d.matched_rule.expect("rule matched");
        assert_eq!(matched.index, 0);
        assert!(matched.condition.contains("domain: example.com"));

        // Second rule matches.
        let d = router.decide_trace(&Address::Ipv4([10, 1, 2, 3]), None);
        assert_eq!(d.action, RouteAction::Route("proxy".to_owned()));
        assert_eq!(d.matched_rule.as_ref().unwrap().index, 1);
        assert!(d
            .matched_rule
            .as_ref()
            .unwrap()
            .condition
            .contains("ip: 10.0.0.0/8"));
    }

    #[test]
    fn decide_trace_final_action_has_no_matched_rule() {
        let router = rs(vec![], RouteAction::Direct);
        let d = router.decide_trace(&Address::Domain("unmatched.example".to_owned()), None);
        assert_eq!(d.action, RouteAction::Direct);
        assert!(d.matched_rule.is_none());
    }

    #[test]
    fn condition_describe_covers_variants() {
        assert_eq!(
            condition_describe(&RuleCondition::Domain(vec!["a.com".into(), "b.com".into()])),
            "domain: a.com, b.com"
        );
        let ip: IpNet = "192.168.0.0/16".parse().unwrap();
        assert_eq!(
            condition_describe(&RuleCondition::Ip(vec![ip])),
            "ip: 192.168.0.0/16"
        );
        assert_eq!(
            condition_describe(&RuleCondition::And(vec![
                RuleCondition::DomainKeyword(vec!["login".into()]),
                RuleCondition::GeoIp(vec!["CN".into()]),
            ])),
            "and(domain_keyword: login, geoip: CN)"
        );
    }
}
