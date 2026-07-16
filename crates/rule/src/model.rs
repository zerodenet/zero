use ipnet::{Ipv4Net, Ipv6Net};

use crate::{PreparedRuleQuery, RuleMatch};

/// Fixed byte capacity reserved for the optional display name in a future ZRS header.
pub const DISPLAY_NAME_CAPACITY: usize = 64;
/// Maximum encoded name length, leaving one byte for the NUL terminator.
pub const DISPLAY_NAME_MAX_BYTES: usize = DISPLAY_NAME_CAPACITY - 1;
/// Maximum number of source rules accepted by one matcher set.
pub const MAX_RULES: usize = 4_000_000;
/// Maximum UTF-8 byte length accepted for one source rule value.
pub const MAX_RULE_VALUE_BYTES: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleSet {
    pub display_name: Option<String>,
    pub rules: Vec<Rule>,
}

impl RuleSet {
    pub fn new(rules: Vec<Rule>) -> Self {
        Self {
            display_name: None,
            rules,
        }
    }

    pub fn with_display_name(mut self, display_name: impl Into<String>) -> Self {
        self.display_name = Some(display_name.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Rule {
    DomainExact(String),
    DomainSuffix(String),
    DomainKeyword(String),
    Ipv4Cidr(Ipv4Net),
    Ipv6Cidr(Ipv6Net),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Ipv4Range {
    pub start: u32,
    pub end: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Ipv6Range {
    pub start: u128,
    pub end: u128,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledRuleSet {
    pub(crate) display_name: Option<String>,
    pub(crate) domain_exact: Vec<String>,
    pub(crate) domain_suffix: Vec<String>,
    pub(crate) domain_keyword: Vec<String>,
    pub(crate) ipv4_ranges: Vec<Ipv4Range>,
    pub(crate) ipv6_ranges: Vec<Ipv6Range>,
}

impl CompiledRuleSet {
    pub fn is_empty(&self) -> bool {
        self.domain_exact.is_empty()
            && self.domain_suffix.is_empty()
            && self.domain_keyword.is_empty()
            && self.ipv4_ranges.is_empty()
            && self.ipv6_ranges.is_empty()
    }

    pub fn entry_count(&self) -> usize {
        self.domain_exact.len()
            + self.domain_suffix.len()
            + self.domain_keyword.len()
            + self.ipv4_ranges.len()
            + self.ipv6_ranges.len()
    }

    pub fn display_name(&self) -> Option<&str> {
        self.display_name.as_deref()
    }

    pub fn domain_exact(&self) -> &[String] {
        &self.domain_exact
    }

    pub fn domain_suffix(&self) -> &[String] {
        &self.domain_suffix
    }

    pub fn domain_keyword(&self) -> &[String] {
        &self.domain_keyword
    }

    pub fn ipv4_ranges(&self) -> &[Ipv4Range] {
        &self.ipv4_ranges
    }

    pub fn ipv6_ranges(&self) -> &[Ipv6Range] {
        &self.ipv6_ranges
    }

    /// Returns whether any matcher in this set accepts the prepared query facts.
    pub fn matches(&self, query: &PreparedRuleQuery) -> bool {
        self.lookup(query).is_some()
    }

    /// Returns the first semantic rule category that accepts the query.
    pub fn lookup(&self, query: &PreparedRuleQuery) -> Option<RuleMatch> {
        query
            .domain()
            .and_then(|domain| self.lookup_domain(domain))
            .or_else(|| {
                query
                    .destination_ip()
                    .and_then(|address| self.lookup_ip(address))
            })
    }

    fn lookup_domain(&self, domain: &str) -> Option<RuleMatch> {
        if self
            .domain_exact
            .binary_search_by(|candidate| candidate.as_str().cmp(domain))
            .is_ok()
        {
            Some(RuleMatch::DomainExact)
        } else if domain_suffix_match(&self.domain_suffix, domain) {
            Some(RuleMatch::DomainSuffix)
        } else if self
            .domain_keyword
            .iter()
            .any(|keyword| domain.contains(keyword))
        {
            Some(RuleMatch::DomainKeyword)
        } else {
            None
        }
    }

    fn lookup_ip(&self, address: std::net::IpAddr) -> Option<RuleMatch> {
        match address {
            std::net::IpAddr::V4(address) => {
                range_contains(&self.ipv4_ranges, u32::from(address), |range| {
                    (range.start, range.end)
                })
                .then_some(RuleMatch::Ipv4Range)
            }
            std::net::IpAddr::V6(address) => {
                range_contains(&self.ipv6_ranges, u128::from(address), |range| {
                    (range.start, range.end)
                })
                .then_some(RuleMatch::Ipv6Range)
            }
        }
    }
}

fn domain_suffix_match(suffixes: &[String], domain: &str) -> bool {
    let mut candidate = domain;
    loop {
        if suffixes
            .binary_search_by(|suffix| suffix.as_str().cmp(candidate))
            .is_ok()
        {
            return true;
        }
        let Some(dot) = candidate.find('.') else {
            return false;
        };
        candidate = &candidate[dot + 1..];
    }
}

fn range_contains<T, V>(ranges: &[T], value: V, bounds: impl Fn(&T) -> (V, V)) -> bool
where
    V: Copy + Ord,
{
    let index = ranges.partition_point(|range| bounds(range).0 <= value);
    index > 0 && value <= bounds(&ranges[index - 1]).1
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CompileReport {
    pub input_rules: usize,
    pub output_entries: usize,
    pub duplicates_removed: usize,
    pub covered_rules_removed: usize,
    pub ranges_merged: usize,
}
