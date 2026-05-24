use std::path::Path;
#[cfg(feature = "sink-jsonl")]
use std::path::PathBuf;
use std::sync::Arc;

use zero_api::{EventSink, RawApiEvent};
use zero_config::{ApiConfig, EventSinkConfig};

use crate::{ConnectorError, ConnectorResult};

pub(crate) struct ConfiguredEventSink {
    pub(crate) tag: String,
    pub(crate) event_types: Vec<String>,
    pub(crate) source_id: Option<String>,
    sink: Arc<dyn EventSink + Send + Sync>,
}

impl ConfiguredEventSink {
    pub(crate) fn accepts(&self, event: &RawApiEvent) -> bool {
        self.event_types.is_empty()
            || self
                .event_types
                .iter()
                .any(|event_type| event_type == &event.event_type)
    }

    pub(crate) fn publish(
        &self,
        event: &RawApiEvent,
    ) -> zero_api::ApiResult<zero_api::PublishResult> {
        let mut event = event.clone();
        if let Some(source_id) = &self.source_id {
            event.source_id = Some(source_id.clone());
        }
        self.sink.publish(&event)
    }
}

pub(crate) fn build_event_sinks(
    api: &ApiConfig,
    source_dir: Option<&Path>,
) -> ConnectorResult<Vec<ConfiguredEventSink>> {
    api.event_sinks
        .iter()
        .map(|config| build_event_sink(config, source_dir))
        .collect()
}

fn build_event_sink(
    config: &EventSinkConfig,
    source_dir: Option<&Path>,
) -> ConnectorResult<ConfiguredEventSink> {
    match config {
        EventSinkConfig::JsonLines {
            tag,
            path,
            events,
            source_id,
        } => build_json_line_sink(tag, path, events, source_id, source_dir),
        EventSinkConfig::Webhook {
            tag,
            url,
            events,
            source_id,
            api_key,
            api_key_env,
            allow_insecure,
        } => build_webhook_sink(
            tag,
            url,
            events,
            source_id,
            api_key,
            api_key_env,
            *allow_insecure,
        ),
    }
}

#[cfg(feature = "sink-jsonl")]
fn build_json_line_sink(
    tag: &str,
    path: &str,
    events: &[String],
    source_id: &Option<String>,
    source_dir: Option<&Path>,
) -> ConnectorResult<ConfiguredEventSink> {
    use std::fs::OpenOptions;

    let resolved = resolve_path(path, source_dir);
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&resolved)
        .map_err(|source| ConnectorError::OpenJsonLineSink {
            tag: tag.to_owned(),
            path: resolved.display().to_string(),
            source,
        })?;

    Ok(ConfiguredEventSink {
        tag: tag.to_owned(),
        event_types: events.to_vec(),
        source_id: source_id.clone(),
        sink: Arc::new(zero_api::JsonLineEventSink::new(file)),
    })
}

#[cfg(not(feature = "sink-jsonl"))]
fn build_json_line_sink(
    tag: &str,
    _path: &str,
    _events: &[String],
    _source_id: &Option<String>,
    _source_dir: Option<&Path>,
) -> ConnectorResult<ConfiguredEventSink> {
    Err(ConnectorError::FeatureDisabled {
        feature: "sink-jsonl",
        sink_type: "jsonl",
        tag: tag.to_owned(),
    })
}

#[cfg(feature = "webhook")]
fn build_webhook_sink(
    tag: &str,
    url: &str,
    events: &[String],
    source_id: &Option<String>,
    api_key: &Option<String>,
    api_key_env: &Option<String>,
    allow_insecure: bool,
) -> ConnectorResult<ConfiguredEventSink> {
    let key = resolve_api_key(api_key, api_key_env)?;
    let mut config = zero_api::WebhookEventSinkConfig::new(url.to_owned())
        .with_header("authorization", format!("Bearer {key}"));
    if allow_insecure {
        config = config.with_allow_insecure(true);
    }
    let sink = zero_api::WebhookEventSink::with_config(config)?;

    Ok(ConfiguredEventSink {
        tag: tag.to_owned(),
        event_types: events.to_vec(),
        source_id: source_id.clone(),
        sink: Arc::new(sink),
    })
}

#[cfg(not(feature = "webhook"))]
fn build_webhook_sink(
    tag: &str,
    _url: &str,
    _events: &[String],
    _source_id: &Option<String>,
    _api_key: &Option<String>,
    _api_key_env: &Option<String>,
    _allow_insecure: bool,
) -> ConnectorResult<ConfiguredEventSink> {
    Err(ConnectorError::FeatureDisabled {
        feature: "panel-connector",
        sink_type: "webhook",
        tag: tag.to_owned(),
    })
}

#[cfg(feature = "webhook")]
fn resolve_api_key(
    api_key: &Option<String>,
    api_key_env: &Option<String>,
) -> ConnectorResult<String> {
    if let Some(key) = api_key {
        return Ok(key.clone());
    }

    let Some(name) = api_key_env else {
        unreachable!("config validation requires api_key or api_key_env for webhook sinks");
    };
    let value = std::env::var(name).map_err(|source| ConnectorError::ReadApiKeyEnv {
        name: name.clone(),
        source,
    })?;
    if value.trim().is_empty() {
        return Err(ConnectorError::EmptyApiKeyEnv { name: name.clone() });
    }
    Ok(value)
}

#[cfg(feature = "sink-jsonl")]
fn resolve_path(path: &str, source_dir: Option<&Path>) -> PathBuf {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        path
    } else if let Some(source_dir) = source_dir {
        source_dir.join(path)
    } else {
        path
    }
}
