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
        for (i, inbound) in self.inbounds.iter().enumerate() {
            validate_tag("inbound", &inbound.tag, &mut inbound_tags)
                .map_err(|e| ConfigError::InvalidInbound(format!("inbounds[{i}]: {e}")))?;
            validate_inbound_listen(
                &mut inbound_listens,
                &inbound.listen.address,
                inbound.listen.port,
            )
            .map_err(|e| {
                ConfigError::InvalidInbound(format!("inbounds[{i}] `{}`: {e}", inbound.tag))
            })?;
            validate_inbound_protocol(&inbound.protocol).map_err(|e| {
                ConfigError::InvalidInbound(format!("inbounds[{i}] `{}`: {e}", inbound.tag))
            })?;
        }

        let mut outbound_tags = HashSet::new();
        let mut route_target_tags = HashSet::new();
        for (i, outbound) in self.outbounds.iter().enumerate() {
            validate_tag("outbound", &outbound.tag, &mut outbound_tags)
                .map_err(|e| ConfigError::InvalidOutbound(format!("outbounds[{i}]: {e}")))?;
            validate_outbound_protocol(&outbound.protocol).map_err(|e| {
                ConfigError::InvalidOutbound(format!("outbounds[{i}] `{}`: {e}", outbound.tag))
            })?;
            validate_route_target_tag(outbound.tag(), &mut route_target_tags)?;
        }

        let mut outbound_group_tags = HashSet::new();
        for (i, group) in self.outbound_groups.iter().enumerate() {
            validate_tag("outbound group", &group.tag, &mut outbound_group_tags).map_err(|e| {
                ConfigError::InvalidOutboundGroup(format!("outbound_groups[{i}]: {e}"))
            })?;
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

    if let Some(dns) = &runtime.dns {
        validate_dns_config(dns)?;
    }

    Ok(())
}

fn validate_dns_config(dns: &crate::DnsConfig) -> Result<(), ConfigError> {
    let num_servers = dns.servers.len();
    for (i, server) in dns.servers.iter().enumerate() {
        match server {
            crate::DnsServerConfig::Udp { address, .. } if address.trim().is_empty() => {
                return Err(ConfigError::InvalidDns(format!(
                    "dns server {i}: udp address must not be empty"
                )));
            }
            crate::DnsServerConfig::Dot { address, .. } if address.trim().is_empty() => {
                return Err(ConfigError::InvalidDns(format!(
                    "dns server {i}: dot address must not be empty"
                )));
            }
            crate::DnsServerConfig::Doh { url, .. } if url.trim().is_empty() => {
                return Err(ConfigError::InvalidDns(format!(
                    "dns server {i}: doh url must not be empty"
                )));
            }
            _ => {}
        }
    }

    if let Some(cache) = &dns.cache {
        if cache.max_entries == 0 {
            return Err(ConfigError::InvalidDns(
                "`dns.cache.max_entries` must be greater than 0".to_owned(),
            ));
        }
    }

    if let Some(fake_ip) = &dns.fake_ip {
        let cidr: Result<ipnet::IpNet, _> = fake_ip.cidr.parse();
        match cidr {
            Ok(net) => {
                let (min_prefix, label) = match net {
                    ipnet::IpNet::V4(_) => (30, "/30 (4 addresses)"),
                    ipnet::IpNet::V6(_) => (120, "/120 (256 addresses)"),
                };
                if net.prefix_len() > min_prefix {
                    return Err(ConfigError::InvalidDns(format!(
                        "`dns.fake_ip.cidr` prefix length is too large for a fake IP pool; \
                         minimum is {label}",
                    )));
                }
            }
            Err(_) => {
                return Err(ConfigError::InvalidDns(format!(
                    "`dns.fake_ip.cidr` is not a valid CIDR: {}",
                    fake_ip.cidr
                )));
            }
        }
        if fake_ip.ttl_seconds == 0 {
            return Err(ConfigError::InvalidDns(
                "`dns.fake_ip.ttl_seconds` must be greater than 0".to_owned(),
            ));
        }
    }

    for (i, route) in dns.routes.iter().enumerate() {
        if route.domain.trim().is_empty() {
            return Err(ConfigError::InvalidDns(format!(
                "dns route {i}: domain must not be empty"
            )));
        }
        if route.server != "system" {
            if let Ok(idx) = route.server.parse::<usize>() {
                if idx >= num_servers {
                    return Err(ConfigError::InvalidDns(format!(
                        "dns route {i}: server index {idx} out of range (0-{})",
                        num_servers.saturating_sub(1)
                    )));
                }
            } else {
                return Err(ConfigError::InvalidDns(format!(
                    "dns route {i}: server must be \"system\" or a number (0-{})",
                    num_servers.saturating_sub(1)
                )));
            }
        }
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
