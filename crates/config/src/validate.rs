use std::collections::HashSet;

use crate::{ConfigError, ModeConfig, RuntimeConfig, RuntimeOptionsConfig};

mod api;
mod group;
mod protocol;
mod route;

use api::validate_api;
use group::validate_group_reference_graph;
use protocol::{validate_inbound_protocol, validate_outbound_protocol};
use route::validate_route_target_tag;

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
            validate_inbound_protocol(&inbound.protocol)?;
        }

        let mut outbound_tags = HashSet::new();
        let mut route_target_tags = HashSet::new();
        for outbound in &self.outbounds {
            validate_tag("outbound", &outbound.tag, &mut outbound_tags)?;
            validate_outbound_protocol(&outbound.protocol)?;
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
        validate_api(&self.api)?;

        Ok(())
    }
}

pub(crate) fn validate_tag(
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
