use std::collections::HashSet;

use crate::{ConfigError, RouteActionConfig, RouteConfig, RouteRuleConfig, RuntimeConfig};

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
        for outbound in &self.outbounds {
            validate_tag("outbound", &outbound.tag, &mut outbound_tags)?;
        }

        self.route.validate(&outbound_tags)?;
        let _ = self.route.compile()?;

        Ok(())
    }
}

impl RouteConfig {
    pub(crate) fn validate(&self, outbound_tags: &HashSet<&str>) -> Result<(), ConfigError> {
        for rule in &self.rules {
            rule.validate(outbound_tags)?;
        }

        validate_route_action(&self.final_action, outbound_tags)
    }
}

impl RouteRuleConfig {
    pub(crate) fn validate(&self, outbound_tags: &HashSet<&str>) -> Result<(), ConfigError> {
        let _ = self.condition.compile()?;
        validate_route_action(&self.action, outbound_tags)
    }
}

fn validate_tag<'a>(
    scope: &'static str,
    tag: &'a str,
    seen: &mut HashSet<&'a str>,
) -> Result<(), ConfigError> {
    if tag.trim().is_empty() {
        return Err(ConfigError::EmptyTag { scope });
    }

    if !seen.insert(tag) {
        return Err(ConfigError::DuplicateTag {
            scope,
            tag: tag.to_owned(),
        });
    }

    Ok(())
}

fn validate_route_action(
    action: &RouteActionConfig,
    outbound_tags: &HashSet<&str>,
) -> Result<(), ConfigError> {
    let Some(outbound) = action.outbound_ref() else {
        return Ok(());
    };

    if outbound.trim().is_empty() {
        return Err(ConfigError::InvalidRouteAction(
            "`route` action requires a non-empty outbound tag".to_owned(),
        ));
    }

    if !outbound_tags.contains(outbound) {
        return Err(ConfigError::UndefinedOutboundTag {
            tag: outbound.to_owned(),
        });
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
