use std::collections::HashSet;

use crate::{ApiConfig, ConfigError, ControlApiConfig, EventSinkConfig};

use super::validate_tag;

pub(super) fn validate_api(api: &ApiConfig) -> Result<(), ConfigError> {
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
