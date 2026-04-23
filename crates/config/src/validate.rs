use std::collections::{HashMap, HashSet};

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
        }

        let mut group_target_tags = outbound_tags.clone();
        group_target_tags.extend(outbound_group_tags.iter().cloned());

        for group in &self.outbound_groups {
            group.validate(&group_target_tags)?;
        }
        validate_group_reference_graph(&self.outbound_groups)?;

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
    fn validate(&self, target_tags: &HashSet<String>) -> Result<(), ConfigError> {
        match &self.group {
            OutboundGroupKind::Selector {
                outbounds,
                default,
                selected,
            } => {
                validate_group_outbounds("selector", outbounds, target_tags)?;

                if let Some(default) = default {
                    validate_selector_choice("default", default, outbounds)?;
                }

                if let Some(selected) = selected {
                    validate_selector_choice("selected", selected, outbounds)?;
                }

                Ok(())
            }
            OutboundGroupKind::Fallback { outbounds } => {
                validate_group_outbounds("fallback", outbounds, target_tags)
            }
            OutboundGroupKind::UrlTest {
                outbounds,
                url,
                interval_seconds,
            } => {
                validate_group_outbounds("urltest", outbounds, target_tags)?;

                if url.trim().is_empty() {
                    return Err(ConfigError::InvalidOutboundGroup(
                        "`urltest` group requires a non-empty `url`".to_owned(),
                    ));
                }

                if !url.starts_with("http://") {
                    return Err(ConfigError::InvalidOutboundGroup(
                        "`urltest` group currently only supports `http://` probe urls".to_owned(),
                    ));
                }

                if *interval_seconds == 0 {
                    return Err(ConfigError::InvalidOutboundGroup(
                        "`urltest` group `interval_seconds` must be greater than 0".to_owned(),
                    ));
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

fn validate_group_outbounds(
    kind: &'static str,
    outbounds: &[String],
    target_tags: &HashSet<String>,
) -> Result<(), ConfigError> {
    if outbounds.is_empty() {
        return Err(ConfigError::InvalidOutboundGroup(format!(
            "`{kind}` group requires at least one outbound"
        )));
    }

    for outbound in outbounds {
        validate_group_member_tag(kind, outbound, target_tags)?;
    }

    Ok(())
}

fn validate_group_member_tag(
    kind: &'static str,
    tag: &str,
    target_tags: &HashSet<String>,
) -> Result<(), ConfigError> {
    if tag.trim().is_empty() {
        return Err(ConfigError::InvalidOutboundGroup(format!(
            "`{kind}` group does not allow empty target tags"
        )));
    }

    if !target_tags.contains(tag) {
        return Err(ConfigError::InvalidOutboundGroup(format!(
            "`{kind}` group references undefined target `{tag}`"
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

fn validate_group_reference_graph(groups: &[OutboundGroupConfig]) -> Result<(), ConfigError> {
    let group_map = groups
        .iter()
        .map(|group| (group.tag.as_str(), group))
        .collect::<HashMap<_, _>>();
    let mut visited = HashSet::new();
    let mut stack = Vec::new();

    for group in groups {
        validate_group_reference_target(group.tag(), &group_map, &mut visited, &mut stack)?;
    }

    Ok(())
}

fn validate_group_reference_target<'a>(
    tag: &'a str,
    group_map: &HashMap<&'a str, &'a OutboundGroupConfig>,
    visited: &mut HashSet<&'a str>,
    stack: &mut Vec<&'a str>,
) -> Result<(), ConfigError> {
    if visited.contains(tag) {
        return Ok(());
    }

    if let Some(index) = stack.iter().position(|current| *current == tag) {
        let mut cycle = stack[index..].to_vec();
        cycle.push(tag);
        return Err(ConfigError::InvalidOutboundGroup(format!(
            "group reference cycle detected: {}",
            cycle.join(" -> ")
        )));
    }

    let Some(group) = group_map.get(tag) else {
        return Ok(());
    };

    stack.push(tag);
    for member in group.group.members() {
        if group_map.contains_key(member.as_str()) {
            validate_group_reference_target(member.as_str(), group_map, visited, stack)?;
        }
    }
    stack.pop();
    visited.insert(tag);

    Ok(())
}
