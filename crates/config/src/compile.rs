use zero_router::{RouteAction, Rule, RuleCondition, RuleSet};

use crate::{ConfigError, RouteActionConfig, RouteConfig, RouteRuleConfig, RuleConditionConfig};

impl RouteConfig {
    pub fn compile(&self) -> Result<RuleSet, ConfigError> {
        let mut rules = Vec::with_capacity(self.rules.len());

        for rule in &self.rules {
            rules.push(rule.compile()?);
        }

        Ok(RuleSet::new(rules, self.final_action.compile()))
    }
}

impl RouteRuleConfig {
    pub(crate) fn compile(&self) -> Result<Rule, ConfigError> {
        Ok(Rule {
            condition: self.condition.compile()?,
            action: self.action.compile(),
        })
    }
}

impl RuleConditionConfig {
    pub(crate) fn compile(&self) -> Result<RuleCondition, ConfigError> {
        match self {
            Self::Domain { values } => {
                validate_domain_values(values)?;
                Ok(RuleCondition::Domain(values.clone()))
            }
            Self::Ip { values } => {
                if values.is_empty() {
                    return Err(ConfigError::InvalidRuleCondition(
                        "`ip` condition requires at least one CIDR".to_owned(),
                    ));
                }

                Ok(RuleCondition::Ip(values.clone()))
            }
            Self::And { items } => compile_nested_condition("and", items, RuleCondition::And),
            Self::Or { items } => compile_nested_condition("or", items, RuleCondition::Or),
        }
    }
}

impl RouteActionConfig {
    pub fn compile(&self) -> RouteAction {
        match self {
            Self::Direct => RouteAction::Direct,
            Self::Reject => RouteAction::Reject,
            Self::Route { outbound } => RouteAction::Route(outbound.clone()),
        }
    }

    pub(crate) fn outbound_ref(&self) -> Option<&str> {
        match self {
            Self::Route { outbound } => Some(outbound),
            Self::Direct | Self::Reject => None,
        }
    }
}

fn validate_domain_values(values: &[String]) -> Result<(), ConfigError> {
    if values.is_empty() {
        return Err(ConfigError::InvalidRuleCondition(
            "`domain` condition requires at least one value".to_owned(),
        ));
    }

    if values.iter().any(|value| value.trim().is_empty()) {
        return Err(ConfigError::InvalidRuleCondition(
            "`domain` condition does not allow empty values".to_owned(),
        ));
    }

    Ok(())
}

fn compile_nested_condition<F>(
    kind: &'static str,
    items: &[RuleConditionConfig],
    wrap: F,
) -> Result<RuleCondition, ConfigError>
where
    F: Fn(Vec<RuleCondition>) -> RuleCondition,
{
    if items.is_empty() {
        return Err(ConfigError::InvalidRuleCondition(format!(
            "`{kind}` condition requires at least one nested condition"
        )));
    }

    let mut compiled = Vec::with_capacity(items.len());
    for item in items {
        compiled.push(item.compile()?);
    }

    Ok(wrap(compiled))
}
