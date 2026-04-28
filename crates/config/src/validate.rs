use std::collections::{HashMap, HashSet};

use crate::{
    ApiConfig, ConfigError, ControlApiConfig, EventSinkConfig, InboundProtocolConfig, ModeConfig,
    OutboundGroupConfig, OutboundGroupKind, OutboundProtocolConfig, RouteActionConfig, RouteConfig,
    RouteRuleConfig, RouteRuleSetConfig, RuleConditionConfig, RuleSetSourceType, RuntimeConfig,
    RuntimeOptionsConfig, Socks5UserConfig, VlessUserConfig,
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

fn validate_api(api: &ApiConfig) -> Result<(), ConfigError> {
    let mut sink_tags = HashSet::new();
    for sink in &api.event_sinks {
        validate_tag("api event sink", sink.tag(), &mut sink_tags)?;
        validate_event_sink_events(sink.tag(), sink.events())?;
        if let Some(source_id) = sink.source_id() {
            validate_optional_non_empty("event sink source_id", source_id)?;
        }

        match sink {
            EventSinkConfig::JsonLines { path, .. } => {
                if path.trim().is_empty() {
                    return Err(ConfigError::InvalidApi(
                        "`jsonl` event sink requires a non-empty `path`".to_owned(),
                    ));
                }
            }
            EventSinkConfig::Webhook {
                url,
                api_key,
                api_key_env,
                allow_insecure,
                ..
            } => {
                validate_webhook_url(url, *allow_insecure)?;
                validate_api_key_fields("webhook event sink", api_key, api_key_env)?;
            }
        }
    }

    validate_control_api(&api.control)
}

fn validate_event_sink_events(tag: &str, events: &[String]) -> Result<(), ConfigError> {
    let mut seen = HashSet::new();
    for event in events {
        if event.trim().is_empty() {
            return Err(ConfigError::InvalidApi(format!(
                "event sink `{tag}` contains an empty event type"
            )));
        }

        if !zero_api::event_type::is_known(event) {
            return Err(ConfigError::InvalidApi(format!(
                "event sink `{tag}` references unknown event type `{event}`"
            )));
        }

        if !seen.insert(event.as_str()) {
            return Err(ConfigError::InvalidApi(format!(
                "event sink `{tag}` contains duplicate event type `{event}`"
            )));
        }
    }
    Ok(())
}

fn validate_webhook_url(url: &str, allow_insecure: bool) -> Result<(), ConfigError> {
    if url.trim().is_empty() {
        return Err(ConfigError::InvalidApi(
            "`webhook` event sink requires a non-empty `url`".to_owned(),
        ));
    }

    if url.starts_with("https://") {
        return Ok(());
    }

    if url.starts_with("http://") {
        if allow_insecure {
            return Ok(());
        }

        return Err(ConfigError::InvalidApi(
            "`http://` webhook urls require `allow_insecure: true`".to_owned(),
        ));
    }

    Err(ConfigError::InvalidApi(
        "`webhook` event sink `url` must start with `https://` or `http://`".to_owned(),
    ))
}

fn validate_control_api(control: &ControlApiConfig) -> Result<(), ConfigError> {
    let has_control_fields =
        control.listen.is_some() || control.api_key.is_some() || control.api_key_env.is_some();
    if !control.enabled {
        if has_control_fields {
            return Err(ConfigError::InvalidApi(
                "`api.control` fields require `enabled: true`".to_owned(),
            ));
        }

        return Ok(());
    }

    if control.listen.is_none() {
        return Err(ConfigError::InvalidApi(
            "`api.control.enabled` requires `listen`".to_owned(),
        ));
    }

    validate_api_key_fields("api control", &control.api_key, &control.api_key_env)
}

fn validate_api_key_fields(
    scope: &'static str,
    api_key: &Option<String>,
    api_key_env: &Option<String>,
) -> Result<(), ConfigError> {
    if api_key.is_none() && api_key_env.is_none() {
        return Err(ConfigError::InvalidApi(format!(
            "`{scope}` requires `api_key` or `api_key_env`"
        )));
    }

    if api_key.is_some() && api_key_env.is_some() {
        return Err(ConfigError::InvalidApi(format!(
            "`{scope}` must not set both `api_key` and `api_key_env`"
        )));
    }

    if let Some(value) = api_key {
        validate_optional_non_empty("api_key", value)?;
    }
    if let Some(value) = api_key_env {
        validate_optional_non_empty("api_key_env", value)?;
    }

    Ok(())
}

fn validate_optional_non_empty(field: &'static str, value: &str) -> Result<(), ConfigError> {
    if value.trim().is_empty() {
        return Err(ConfigError::InvalidApi(format!(
            "`{field}` must not be empty"
        )));
    }

    Ok(())
}

fn validate_inbound_protocol(protocol: &InboundProtocolConfig) -> Result<(), ConfigError> {
    match protocol {
        InboundProtocolConfig::Socks5 { users } => validate_socks5_users("socks5 inbound", users),
        InboundProtocolConfig::Mixed { socks5_users } => {
            validate_socks5_users("mixed inbound socks5", socks5_users)
        }
        InboundProtocolConfig::HttpConnect => Ok(()),
        InboundProtocolConfig::Vless { users } => validate_vless_users(users),
    }
}

fn validate_outbound_protocol(protocol: &OutboundProtocolConfig) -> Result<(), ConfigError> {
    match protocol {
        OutboundProtocolConfig::Socks5 {
            username, password, ..
        } => validate_socks5_outbound_auth(username.as_deref(), password.as_deref()),
        OutboundProtocolConfig::Vless { server, port, id } => {
            validate_outbound_endpoint("vless", server, *port)?;
            validate_uuid_literal(id).map_err(|message| {
                ConfigError::InvalidOutbound(format!("`vless` outbound `id` {message}"))
            })
        }
        OutboundProtocolConfig::Direct | OutboundProtocolConfig::Block => Ok(()),
    }
}

fn validate_vless_users(users: &[VlessUserConfig]) -> Result<(), ConfigError> {
    if users.is_empty() {
        return Err(ConfigError::InvalidInbound(
            "`vless` inbound requires at least one user".to_owned(),
        ));
    }

    let mut seen = HashSet::new();
    for user in users {
        validate_uuid_literal(&user.id).map_err(|message| {
            ConfigError::InvalidInbound(format!("`vless` inbound user `id` {message}"))
        })?;

        if !seen.insert(normalize_uuid_key(&user.id)) {
            return Err(ConfigError::InvalidInbound(
                "`vless` inbound contains duplicate user id".to_owned(),
            ));
        }

        if let Some(credential_id) = &user.credential_id {
            validate_inbound_optional_non_empty("vless credential_id", credential_id)?;
        }
        if let Some(principal_key) = &user.principal_key {
            validate_inbound_optional_non_empty("vless principal_key", principal_key)?;
        }
    }

    Ok(())
}

fn validate_outbound_endpoint(
    protocol: &'static str,
    server: &str,
    port: u16,
) -> Result<(), ConfigError> {
    if server.trim().is_empty() {
        return Err(ConfigError::InvalidOutbound(format!(
            "`{protocol}` outbound requires a non-empty `server`"
        )));
    }

    if port == 0 {
        return Err(ConfigError::InvalidOutbound(format!(
            "`{protocol}` outbound `port` must be greater than 0"
        )));
    }

    Ok(())
}

fn validate_socks5_users(
    scope: &'static str,
    users: &[Socks5UserConfig],
) -> Result<(), ConfigError> {
    let mut seen = HashSet::new();

    for user in users {
        validate_socks5_credential_part(scope, "username", &user.username)?;
        validate_socks5_credential_part(scope, "password", &user.password)?;
        if !seen.insert(user.username.as_str()) {
            return Err(ConfigError::InvalidInbound(format!(
                "`{scope}` contains duplicate username `{}`",
                user.username
            )));
        }
    }

    Ok(())
}

fn validate_socks5_outbound_auth(
    username: Option<&str>,
    password: Option<&str>,
) -> Result<(), ConfigError> {
    match (username, password) {
        (None, None) => Ok(()),
        (Some(username), Some(password)) => {
            validate_socks5_outbound_credential_part("username", username)?;
            validate_socks5_outbound_credential_part("password", password)
        }
        _ => Err(ConfigError::InvalidOutbound(
            "`socks5` outbound requires both `username` and `password`, or neither".to_owned(),
        )),
    }
}

fn validate_socks5_outbound_credential_part(
    field: &'static str,
    value: &str,
) -> Result<(), ConfigError> {
    let len = value.len();
    if len == 0 {
        return Err(ConfigError::InvalidOutbound(format!(
            "`socks5` outbound `{field}` must not be empty"
        )));
    }

    if len > u8::MAX as usize {
        return Err(ConfigError::InvalidOutbound(format!(
            "`socks5` outbound `{field}` must be at most 255 bytes"
        )));
    }

    Ok(())
}

fn validate_socks5_credential_part(
    scope: &'static str,
    field: &'static str,
    value: &str,
) -> Result<(), ConfigError> {
    let len = value.len();
    if len == 0 {
        return Err(ConfigError::InvalidInbound(format!(
            "`{scope}` `{field}` must not be empty"
        )));
    }

    if len > u8::MAX as usize {
        return Err(ConfigError::InvalidInbound(format!(
            "`{scope}` `{field}` must be at most 255 bytes"
        )));
    }

    Ok(())
}

fn validate_inbound_optional_non_empty(
    field: &'static str,
    value: &str,
) -> Result<(), ConfigError> {
    if value.trim().is_empty() {
        return Err(ConfigError::InvalidInbound(format!(
            "`{field}` must not be empty"
        )));
    }

    Ok(())
}

fn validate_uuid_literal(value: &str) -> Result<(), &'static str> {
    let value = value.trim();
    let mut digits = 0;

    for (index, byte) in value.bytes().enumerate() {
        if byte == b'-' {
            if value.len() != 36 || !matches!(index, 8 | 13 | 18 | 23) {
                return Err("must be a canonical UUID or 32 hex digits");
            }
            continue;
        }

        if !byte.is_ascii_hexdigit() {
            return Err("must contain only hex digits");
        }

        digits += 1;
    }

    if digits == 32 {
        Ok(())
    } else {
        Err("must contain 32 hex digits")
    }
}

fn normalize_uuid_key(value: &str) -> String {
    value
        .bytes()
        .filter(|byte| *byte != b'-')
        .map(|byte| char::from(byte.to_ascii_lowercase()))
        .collect()
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
