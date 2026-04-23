use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use ipnet::IpNet;
use zero_core::Address;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleCondition {
    Domain(Vec<String>),
    Ip(Vec<IpNet>),
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

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuleSet {
    pub rules: Vec<Rule>,
    pub final_action: RouteAction,
}

impl RuleSet {
    pub fn new(rules: Vec<Rule>, final_action: RouteAction) -> Self {
        Self {
            rules,
            final_action,
        }
    }

    pub fn decide_ref(&self, address: &Address) -> &RouteAction {
        self.rules
            .iter()
            .find(|rule| condition_matches(&rule.condition, address))
            .map(|rule| &rule.action)
            .unwrap_or(&self.final_action)
    }

    pub fn decide(&self, address: &Address) -> RouteAction {
        self.decide_ref(address).clone()
    }
}

fn condition_matches(condition: &RuleCondition, address: &Address) -> bool {
    match condition {
        RuleCondition::Domain(patterns) => match address {
            Address::Domain(domain) => patterns
                .iter()
                .any(|pattern| domain_matches(pattern, domain)),
            _ => false,
        },
        RuleCondition::Ip(networks) => match address_to_ip(address) {
            Some(ip) => networks.iter().any(|network| network.contains(&ip)),
            None => false,
        },
        RuleCondition::And(conditions) => conditions
            .iter()
            .all(|condition| condition_matches(condition, address)),
        RuleCondition::Or(conditions) => conditions
            .iter()
            .any(|condition| condition_matches(condition, address)),
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
