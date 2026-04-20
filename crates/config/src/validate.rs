use std::collections::HashSet;

use crate::{
    ConfigError, ModeConfig, OutboundGroupConfig, OutboundGroupKind, RouteActionConfig,
    RouteConfig, RouteRuleConfig, RuntimeConfig, RuntimeOptionsConfig,
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

        self.route.validate(&route_target_tags)?;
        validate_runtime(&self.runtime)?;
        validate_mode(&self.mode, &route_target_tags)?;
        let _ = self.route.compile()?;

        Ok(())
    }
}

impl RouteConfig {
    pub(crate) fn validate(&self, route_target_tags: &HashSet<String>) -> Result<(), ConfigError> {
        for rule in &self.rules {
            rule.validate(route_target_tags)?;
        }

        validate_route_action(&self.final_action, route_target_tags)
    }
}

impl RouteRuleConfig {
    pub(crate) fn validate(&self, route_target_tags: &HashSet<String>) -> Result<(), ConfigError> {
        let _ = self.condition.compile()?;
        validate_route_action(&self.action, route_target_tags)
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
