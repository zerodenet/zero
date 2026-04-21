use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use ipnet::IpNet;
use zero_router::RuleCondition;

use crate::{ConfigError, RouteRuleSetConfig, RuleSetFormatConfig, RuleSetSourceType};

pub type CompiledRuleSets = HashMap<String, RuleCondition>;

pub fn compile_rule_sets(
    rule_sets: &[RouteRuleSetConfig],
    base_dir: Option<&Path>,
) -> Result<CompiledRuleSets, ConfigError> {
    let mut compiled = HashMap::with_capacity(rule_sets.len());

    for rule_set in rule_sets {
        compiled.insert(rule_set.tag.clone(), compile_rule_set(rule_set, base_dir)?);
    }

    Ok(compiled)
}

fn compile_rule_set(
    rule_set: &RouteRuleSetConfig,
    base_dir: Option<&Path>,
) -> Result<RuleCondition, ConfigError> {
    let raw = load_rule_set_source(rule_set, base_dir)?;
    let values = raw
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#') && !line.starts_with("//"))
        .collect::<Vec<_>>();

    if values.is_empty() {
        return Err(ConfigError::InvalidRuleSet(format!(
            "rule set `{}` does not contain any entries",
            rule_set.tag
        )));
    }

    match rule_set.format {
        RuleSetFormatConfig::DomainList => Ok(RuleCondition::Domain(
            values.into_iter().map(ToOwned::to_owned).collect(),
        )),
        RuleSetFormatConfig::CidrList => {
            let mut networks = Vec::with_capacity(values.len());
            for value in values {
                let network = value.parse::<IpNet>().map_err(|error| {
                    ConfigError::InvalidRuleSet(format!(
                        "rule set `{}` contains invalid CIDR `{value}`: {error}",
                        rule_set.tag
                    ))
                })?;
                networks.push(network);
            }

            Ok(RuleCondition::Ip(networks))
        }
    }
}

fn load_rule_set_source(
    rule_set: &RouteRuleSetConfig,
    base_dir: Option<&Path>,
) -> Result<String, ConfigError> {
    match rule_set.source_type {
        RuleSetSourceType::File => {
            let resolved = resolve_rule_set_path(&rule_set.path, base_dir);
            fs::read_to_string(&resolved).map_err(|source| ConfigError::ReadRuleSet {
                path: resolved.display().to_string(),
                source,
            })
        }
    }
}

fn resolve_rule_set_path(path: &str, base_dir: Option<&Path>) -> PathBuf {
    let candidate = Path::new(path);
    if candidate.is_absolute() {
        return candidate.to_path_buf();
    }

    match base_dir {
        Some(base_dir) => base_dir.join(candidate),
        None => candidate.to_path_buf(),
    }
}
