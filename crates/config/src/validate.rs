use std::collections::HashSet;

use crate::{
    ConfigError, ModeConfig, OutboundGroupConfig, OutboundGroupKind, RouteActionConfig,
    RouteConfig, RouteRuleConfig, RouteRuleSetConfig, RuleConditionConfig, RuleSetSourceType,
    RuntimeConfig, RuntimeOptionsConfig,
};

impl RuntimeConfig {
    pub fn validate(&self) -> Result<(), ConfigError> {
        let mut inbound_tags = HashSet::new();
        let mut inbound_listens = HashSet::new();
        for inbound in &self.inbounds {
            validate_tag("inbound", &inbound.tag, &mut inbound_tags)?;
            validate_inbound_listen(
                &mut inbound_listens,
                &inbound.listen.address,
                inbound.listen.port,
            )?;
        }

        let mut outbound_tags = HashSet::new();
        let mut route_target_tags = HashSet::new();
        for outbound in &self.outbounds {
            validate_tag("outbound", &outbound.tag, &mut outbound_tags)?;
            validate_route_target_tag(outbound.tag(), &mut route_target_tags)?;
        }

        let mut outbound_group_tags = HashSet::new();
        for group in &self.outbound_groups {
            validate_tag("outbound group", &group.tag, &mut outbound_group_tags)?;
            validate_route_target_tag(group.tag(), &mut route_target_tags)?;
            group.validate(&outbound_tags)?;
        }

        self.route.validate(&route_target_tags, self.source_dir())?;
        validate_runtime(&self.runtime)?;
        validate_mode(&self.mode, &route_target_tags)?;

        Ok(())
    }
}

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
            Self::And { items } => validate_nested_condition("and", items, rule_set_tags),
            Self::Or { items } => validate_nested_condition("or", items, rule_set_tags),
        }
    }
}

impl OutboundGroupConfig {
    fn validate(&self, outbound_tags: &HashSet<String>) -> Result<(), ConfigError> {
        match &self.group {
            OutboundGroupKind::Selector {
                outbounds,
                default,
                selected,
            } => {
                if outbounds.is_empty() {
                    return Err(ConfigError::InvalidOutboundGroup(
                        "`selector` group requires at least one outbound".to_owned(),
                    ));
                }

                for outbound in outbounds {
                    validate_group_member_tag(outbound, outbound_tags)?;
                }

                if let Some(default) = default {
                    validate_selector_choice("default", default, outbounds)?;
                }

                if let Some(selected) = selected {
                    validate_selector_choice("selected", selected, outbounds)?;
                }

                Ok(())
            }
        }
    }
}

fn validate_tag(
    scope: &'static str,
    tag: &str,
    seen: &mut HashSet<String>,
) -> Result<(), ConfigError> {
    if tag.trim().is_empty() {
        return Err(ConfigError::EmptyTag { scope });
    }

    if !seen.insert(tag.to_owned()) {
        return Err(ConfigError::DuplicateTag {
            scope,
            tag: tag.to_owned(),
        });
    }

    Ok(())
}

fn validate_route_action(
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

fn validate_mode(
    mode: &ModeConfig,
    route_target_tags: &HashSet<String>,
) -> Result<(), ConfigError> {
    match mode {
        ModeConfig::Rule | ModeConfig::Direct => Ok(()),
        ModeConfig::Global { outbound } => {
            if outbound.trim().is_empty() {
                return Err(ConfigError::InvalidMode(
                    "`global` mode requires a non-empty outbound target".to_owned(),
                ));
            }

            if !route_target_tags.contains(outbound) {
                return Err(ConfigError::UndefinedRouteTargetTag {
                    tag: outbound.to_owned(),
                });
            }

            Ok(())
        }
    }
}

fn validate_runtime(runtime: &RuntimeOptionsConfig) -> Result<(), ConfigError> {
    if runtime.udp_upstream_idle_timeout_seconds == 0 {
        return Err(ConfigError::InvalidRuntime(
            "`runtime.udp_upstream_idle_timeout_seconds` must be greater than 0".to_owned(),
        ));
    }

    Ok(())
}

fn validate_route_target_tag(tag: &str, seen: &mut HashSet<String>) -> Result<(), ConfigError> {
    if !seen.insert(tag.to_owned()) {
        return Err(ConfigError::DuplicateRouteTargetTag {
            tag: tag.to_owned(),
        });
    }

    Ok(())
}

fn validate_group_member_tag(
    tag: &str,
    outbound_tags: &HashSet<String>,
) -> Result<(), ConfigError> {
    if tag.trim().is_empty() {
        return Err(ConfigError::InvalidOutboundGroup(
            "`selector` group does not allow empty outbound tags".to_owned(),
        ));
    }

    if !outbound_tags.contains(tag) {
        return Err(ConfigError::InvalidOutboundGroup(format!(
            "`selector` group references undefined outbound `{tag}`"
        )));
    }

    Ok(())
}

fn validate_selector_choice(
    field: &'static str,
    value: &str,
    outbounds: &[String],
) -> Result<(), ConfigError> {
    if value.trim().is_empty() {
        return Err(ConfigError::InvalidOutboundGroup(format!(
            "`selector` group `{field}` must not be empty"
        )));
    }

    if !outbounds.iter().any(|outbound| outbound == value) {
        return Err(ConfigError::InvalidOutboundGroup(format!(
            "`selector` group `{field}` must reference one of its `outbounds`"
        )));
    }

    Ok(())
}

fn validate_inbound_listen(
    seen: &mut HashSet<(String, u16)>,
    address: &str,
    port: u16,
) -> Result<(), ConfigError> {
    let key = (address.to_owned(), port);

    if !seen.insert(key.clone()) {
        return Err(ConfigError::DuplicateInboundListen {
            address: key.0,
            port: key.1,
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
