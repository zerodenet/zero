//! Stable exchange protocol for rules defined by Zero itself.

use ipnet::{Ipv4Net, Ipv6Net};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{Rule, RuleSet};

pub const ZERO_RULE_IR_VERSION: u32 = 1;
pub const MAX_IR_BYTES: usize = 64 * 1024 * 1024;
pub use crate::{MAX_RULES, MAX_RULE_VALUE_BYTES};

#[derive(Debug, Error)]
pub enum RuleProtocolError {
    #[error("Zero Rule IR is {actual} bytes; maximum is {maximum}")]
    InputTooLarge { actual: usize, maximum: usize },
    #[error("invalid Zero Rule IR JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported Zero Rule IR version {actual}; expected {expected}")]
    UnsupportedVersion { actual: u32, expected: u32 },
    #[error("Zero Rule IR contains {actual} rules; maximum is {maximum}")]
    TooManyRules { actual: usize, maximum: usize },
    #[error("rule {index} value is {actual} bytes; maximum is {maximum}")]
    RuleValueTooLong {
        index: usize,
        actual: usize,
        maximum: usize,
    },
    #[error("rule {index} contains invalid IPv4 CIDR `{value}`: {source}")]
    InvalidIpv4 {
        index: usize,
        value: String,
        source: ipnet::AddrParseError,
    },
    #[error("rule {index} contains invalid IPv6 CIDR `{value}`: {source}")]
    InvalidIpv6 {
        index: usize,
        value: String,
        source: ipnet::AddrParseError,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RuleDocument {
    version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    rules: Vec<WireRule>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
enum WireRule {
    DomainExact(String),
    DomainSuffix(String),
    DomainKeyword(String),
    Ipv4Cidr(String),
    Ipv6Cidr(String),
}

pub fn decode_json(input: &[u8]) -> Result<RuleSet, RuleProtocolError> {
    if input.len() > MAX_IR_BYTES {
        return Err(RuleProtocolError::InputTooLarge {
            actual: input.len(),
            maximum: MAX_IR_BYTES,
        });
    }
    let document: RuleDocument = serde_json::from_slice(input)?;
    if document.version != ZERO_RULE_IR_VERSION {
        return Err(RuleProtocolError::UnsupportedVersion {
            actual: document.version,
            expected: ZERO_RULE_IR_VERSION,
        });
    }
    if document.rules.len() > MAX_RULES {
        return Err(RuleProtocolError::TooManyRules {
            actual: document.rules.len(),
            maximum: MAX_RULES,
        });
    }

    let mut rules = Vec::with_capacity(document.rules.len());
    for (index, rule) in document.rules.into_iter().enumerate() {
        let value = rule.value();
        if value.len() > MAX_RULE_VALUE_BYTES {
            return Err(RuleProtocolError::RuleValueTooLong {
                index,
                actual: value.len(),
                maximum: MAX_RULE_VALUE_BYTES,
            });
        }
        rules.push(rule.into_rule(index)?);
    }
    Ok(RuleSet {
        display_name: document.name,
        rules,
    })
}

pub fn encode_json(rule_set: &RuleSet) -> Result<Vec<u8>, RuleProtocolError> {
    validate_rule_count(rule_set.rules.len())?;
    for (index, rule) in rule_set.rules.iter().enumerate() {
        validate_value(index, WireRule::from(rule).value())?;
    }
    let document = RuleDocument {
        version: ZERO_RULE_IR_VERSION,
        name: rule_set.display_name.clone(),
        rules: rule_set.rules.iter().map(WireRule::from).collect(),
    };
    Ok(serde_json::to_vec_pretty(&document)?)
}

fn validate_rule_count(count: usize) -> Result<(), RuleProtocolError> {
    if count > MAX_RULES {
        return Err(RuleProtocolError::TooManyRules {
            actual: count,
            maximum: MAX_RULES,
        });
    }
    Ok(())
}

fn validate_value(index: usize, value: &str) -> Result<(), RuleProtocolError> {
    if value.len() > MAX_RULE_VALUE_BYTES {
        return Err(RuleProtocolError::RuleValueTooLong {
            index,
            actual: value.len(),
            maximum: MAX_RULE_VALUE_BYTES,
        });
    }
    Ok(())
}

impl WireRule {
    fn value(&self) -> &str {
        match self {
            Self::DomainExact(value)
            | Self::DomainSuffix(value)
            | Self::DomainKeyword(value)
            | Self::Ipv4Cidr(value)
            | Self::Ipv6Cidr(value) => value,
        }
    }

    fn into_rule(self, index: usize) -> Result<Rule, RuleProtocolError> {
        match self {
            Self::DomainExact(value) => Ok(Rule::DomainExact(value)),
            Self::DomainSuffix(value) => Ok(Rule::DomainSuffix(value)),
            Self::DomainKeyword(value) => Ok(Rule::DomainKeyword(value)),
            Self::Ipv4Cidr(value) => {
                value
                    .parse::<Ipv4Net>()
                    .map(Rule::Ipv4Cidr)
                    .map_err(|source| RuleProtocolError::InvalidIpv4 {
                        index,
                        value,
                        source,
                    })
            }
            Self::Ipv6Cidr(value) => {
                value
                    .parse::<Ipv6Net>()
                    .map(Rule::Ipv6Cidr)
                    .map_err(|source| RuleProtocolError::InvalidIpv6 {
                        index,
                        value,
                        source,
                    })
            }
        }
    }
}

impl From<&Rule> for WireRule {
    fn from(rule: &Rule) -> Self {
        match rule {
            Rule::DomainExact(value) => Self::DomainExact(value.clone()),
            Rule::DomainSuffix(value) => Self::DomainSuffix(value.clone()),
            Rule::DomainKeyword(value) => Self::DomainKeyword(value.clone()),
            Rule::Ipv4Cidr(value) => Self::Ipv4Cidr(value.to_string()),
            Rule::Ipv6Cidr(value) => Self::Ipv6Cidr(value.to_string()),
        }
    }
}
