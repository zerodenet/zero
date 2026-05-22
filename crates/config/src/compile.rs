use std::collections::HashMap;
use std::path::Path;

use zero_router::{RouteAction, Rule, RuleCondition, RuleSet};

use crate::rule_sets::compile_rule_sets;
use crate::{ConfigError, RouteActionConfig, RouteConfig, RouteRuleConfig, RuleConditionConfig};

impl RouteConfig {
    pub fn compile(&self, base_dir: Option<&Path>) -> Result<RuleSet, ConfigError> {
        let compiled_rule_sets = compile_rule_sets(&self.rule_sets, base_dir)?;
        let mut rules = Vec::with_capacity(self.rules.len());

        for rule in &self.rules {
            rules.push(rule.compile(&compiled_rule_sets)?);
        }

        if let Some(ref path) = self.geoip_database {
            let resolved = resolve_path(path, base_dir);
            let data = std::fs::read(&resolved).map_err(|source| {
                ConfigError::InvalidRuleSet(format!(
                    "failed to read geoip database at {}: {source}",
                    resolved.display()
                ))
            })?;
            let reader = maxminddb::Reader::from_source(data)
                .map_err(|e| ConfigError::InvalidRuleSet(format!("invalid geoip database: {e}")))?;
            Ok(RuleSet::with_geoip(
                rules,
                self.final_action.compile(),
                std::sync::Arc::new(reader),
            ))
        } else {
            Ok(RuleSet::new(rules, self.final_action.compile()))
        }
    }
}

fn resolve_path(path: &str, base_dir: Option<&Path>) -> std::path::PathBuf {
    let candidate = std::path::Path::new(path);
    if candidate.is_absolute() {
        return candidate.to_path_buf();
    }
    match base_dir {
        Some(base_dir) => base_dir.join(candidate),
        None => candidate.to_path_buf(),
    }
}

impl RouteRuleConfig {
    pub(crate) fn compile(
        &self,
        compiled_rule_sets: &HashMap<String, RuleCondition>,
    ) -> Result<Rule, ConfigError> {
        Ok(Rule {
            condition: self.condition.compile(compiled_rule_sets)?,
            action: self.action.compile(),
        })
    }
}

impl RuleConditionConfig {
    pub(crate) fn compile(
        &self,
        compiled_rule_sets: &HashMap<String, RuleCondition>,
    ) -> Result<RuleCondition, ConfigError> {
        match self {
            Self::Domain { values } => {
                validate_domain_values(values)?;
                Ok(RuleCondition::Domain(values.clone()))
            }
            Self::DomainKeyword { values } => {
                validate_domain_values(values)?;
                Ok(RuleCondition::DomainKeyword(values.clone()))
            }
            Self::DomainRegex { values } => {
                let compiled: Result<Vec<_>, _> = values
                    .iter()
                    .map(|p| zero_router::CompiledRegex::new(p.clone()))
                    .collect();
                match compiled {
                    Ok(regexes) => Ok(RuleCondition::DomainRegex(regexes)),
                    Err(e) => Err(ConfigError::InvalidRuleCondition(format!(
                        "invalid regex pattern: {e}"
                    ))),
                }
            }
            Self::Ip { values } => {
                if values.is_empty() {
                    return Err(ConfigError::InvalidRuleCondition(
                        "`ip` condition requires at least one CIDR".to_owned(),
                    ));
                }

                Ok(RuleCondition::Ip(values.clone()))
            }
            Self::RuleSet { tag } => compiled_rule_sets
                .get(tag)
                .cloned()
                .ok_or_else(|| ConfigError::UndefinedRuleSetTag { tag: tag.clone() }),
            Self::GeoIp { values } => {
                if values.is_empty() {
                    return Err(ConfigError::InvalidRuleCondition(
                        "`geoip` condition requires at least one country code".to_owned(),
                    ));
                }
                Ok(RuleCondition::GeoIp(values.clone()))
            }
            Self::Sni { values } => {
                validate_domain_values(values)?;
                Ok(RuleCondition::Sni(values.clone()))
            }
            Self::And { items } => {
                compile_nested_condition("and", items, compiled_rule_sets, RuleCondition::And)
            }
            Self::Or { items } => {
                compile_nested_condition("or", items, compiled_rule_sets, RuleCondition::Or)
            }
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

    pub(crate) fn target_ref(&self) -> Option<&str> {
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
    compiled_rule_sets: &HashMap<String, RuleCondition>,
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
        compiled.push(item.compile(compiled_rule_sets)?);
    }

    Ok(wrap(compiled))
}
