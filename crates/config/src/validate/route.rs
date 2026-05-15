use std::collections::HashSet;

use crate::{
    ConfigError, RouteActionConfig, RouteConfig, RouteRuleConfig, RouteRuleSetConfig,
    RuleConditionConfig, RuleSetSourceType,
};

use super::validate_tag;

impl RouteConfig {
    pub(crate) fn validate(
        &self,
        route_target_tags: &HashSet<String>,
        base_dir: Option<&std::path::Path>,
    ) -> Result<(), ConfigError> {
        let mut rule_set_tags = HashSet::new();
        for rule_set in &self.rule_sets {
            validate_tag("rule set", &rule_set.tag, &mut rule_set_tags)?;
            rule_set.validate()?;
        }

        for rule in &self.rules {
            rule.validate(route_target_tags, &rule_set_tags)?;
        }

        validate_route_action(&self.final_action, route_target_tags)?;
        let _ = self.compile(base_dir)?;

        Ok(())
    }
}

impl RouteRuleSetConfig {
    fn validate(&self) -> Result<(), ConfigError> {
        match self.source_type {
            RuleSetSourceType::File => {
                if self.path.trim().is_empty() {
                    return Err(ConfigError::InvalidRuleSet(
                        "`file` rule set requires a non-empty `path`".to_owned(),
                    ));
                }
            }
            RuleSetSourceType::Url => {
                if self.url.as_ref().map_or(true, |u| u.trim().is_empty()) {
                    return Err(ConfigError::InvalidRuleSet(
                        "`url` rule set requires a non-empty `url`".to_owned(),
                    ));
                }
                if self.path.trim().is_empty() {
                    return Err(ConfigError::InvalidRuleSet(
                        "`url` rule set requires a non-empty `path` for cache".to_owned(),
                    ));
                }
            }
        }

        Ok(())
    }
}

impl RouteRuleConfig {
    pub(crate) fn validate(
        &self,
        route_target_tags: &HashSet<String>,
        rule_set_tags: &HashSet<String>,
    ) -> Result<(), ConfigError> {
        self.condition.validate(rule_set_tags)?;
        validate_route_action(&self.action, route_target_tags)
    }
}

impl RuleConditionConfig {
    fn validate(&self, rule_set_tags: &HashSet<String>) -> Result<(), ConfigError> {
        match self {
            Self::Domain { values } => {
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
            Self::Ip { values } => {
                if values.is_empty() {
                    return Err(ConfigError::InvalidRuleCondition(
                        "`ip` condition requires at least one CIDR".to_owned(),
                    ));
                }

                Ok(())
            }
            Self::RuleSet { tag } => {
                if tag.trim().is_empty() {
                    return Err(ConfigError::InvalidRuleCondition(
                        "`rule-set` condition requires a non-empty `tag`".to_owned(),
                    ));
                }
                if !rule_set_tags.contains(tag) {
                    return Err(ConfigError::UndefinedRuleSetTag { tag: tag.clone() });
                }
                Ok(())
            }
            Self::DomainKeyword { values } => {
                if values.is_empty() {
                    return Err(ConfigError::InvalidRuleCondition(
                        "`domain-keyword` condition requires at least one value".to_owned(),
                    ));
                }
                Ok(())
            }
            Self::GeoIp { values } => {
                if values.is_empty() {
                    return Err(ConfigError::InvalidRuleCondition(
                        "`geoip` condition requires at least one country code".to_owned(),
                    ));
                }
                Ok(())
            }
            Self::And { items } => validate_nested_condition("and", items, rule_set_tags),
            Self::Or { items } => validate_nested_condition("or", items, rule_set_tags),
        }
    }
}

pub(super) fn validate_route_action(
    action: &RouteActionConfig,
    route_target_tags: &HashSet<String>,
) -> Result<(), ConfigError> {
    let Some(outbound) = action.target_ref() else {
        return Ok(());
    };

    if outbound.trim().is_empty() {
        return Err(ConfigError::InvalidRouteAction(
            "`route` action requires a non-empty outbound tag".to_owned(),
        ));
    }

    if !route_target_tags.contains(outbound) {
        return Err(ConfigError::UndefinedRouteTargetTag {
            tag: outbound.to_owned(),
        });
    }

    Ok(())
}

pub(super) fn validate_route_target_tag(
    tag: &str,
    seen: &mut HashSet<String>,
) -> Result<(), ConfigError> {
    if !seen.insert(tag.to_owned()) {
        return Err(ConfigError::DuplicateRouteTargetTag {
            tag: tag.to_owned(),
        });
    }

    Ok(())
}

fn validate_nested_condition(
    kind: &'static str,
    items: &[RuleConditionConfig],
    rule_set_tags: &HashSet<String>,
) -> Result<(), ConfigError> {
    if items.is_empty() {
        return Err(ConfigError::InvalidRuleCondition(format!(
            "`{kind}` condition requires at least one nested condition"
        )));
    }

    for item in items {
        item.validate(rule_set_tags)?;
    }

    Ok(())
}
