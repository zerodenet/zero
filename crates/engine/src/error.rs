use std::io;

use thiserror::Error;
use zero_config::ConfigError;
use zero_core::Error as CoreError;

#[derive(Debug, Error)]
pub enum EngineError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Core(#[from] CoreError),
    #[error("no inbound listeners are configured")]
    NoInbounds,
    #[error(
        "{kind} `{tag}` uses protocol `{protocol}` but this binary was built without Cargo feature `{feature}`"
    )]
    CompiledFeatureDisabled {
        kind: &'static str,
        tag: String,
        protocol: &'static str,
        feature: &'static str,
    },
    #[error("route or mode references target tag `{tag}` but no such outbound or group exists")]
    MissingRouteTarget { tag: String },
    #[error("engine plan is invalid: {message}")]
    InvalidPlan { message: String },
    #[error("inbound task exited unexpectedly")]
    InboundTaskExited,
    #[error("url_test group `{tag}` is invalid: {message}")]
    InvalidUrlTestGroup { tag: String, message: String },
    #[error("url_test task exited unexpectedly")]
    UrlTestTaskExited,
    #[error("selector group `{tag}` does not exist")]
    SelectorGroupNotFound { tag: String },
    #[error("group `{tag}` is not a selector group")]
    SelectorGroupTypeMismatch { tag: String },
    #[error("selector group `{group_tag}` does not contain target `{target_tag}`")]
    SelectorTargetNotFound {
        group_tag: String,
        target_tag: String,
    },
    #[error("outbound `{tag}` is temporarily unhealthy")]
    UnhealthyOutbound { tag: String },
}
