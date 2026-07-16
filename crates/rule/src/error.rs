use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CompileError {
    #[error("rule set contains {actual} rules; maximum is {maximum}")]
    TooManyRules { actual: usize, maximum: usize },
    #[error("rule-set display name must not be empty")]
    EmptyDisplayName,
    #[error("rule-set display name contains a NUL byte")]
    DisplayNameContainsNul,
    #[error("rule-set display name is {actual} bytes; maximum is {maximum}")]
    DisplayNameTooLong { actual: usize, maximum: usize },
    #[error("rule {index} contains invalid domain `{value}`: {reason}")]
    InvalidDomain {
        index: usize,
        value: String,
        reason: String,
    },
    #[error("rule set does not contain any rules")]
    EmptyRuleSet,
}
