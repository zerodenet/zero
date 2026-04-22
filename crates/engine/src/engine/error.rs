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
    #[error("route or mode references target tag `{tag}` but no such outbound or group exists")]
    MissingRouteTarget { tag: String },
    #[error("inbound task exited unexpectedly")]
    InboundTaskExited,
    #[error("urltest group `{tag}` is invalid: {message}")]
    InvalidUrlTestGroup { tag: String, message: String },
    #[error("urltest task exited unexpectedly")]
    UrlTestTaskExited,
    #[error("selector group `{tag}` does not exist")]
    SelectorGroupNotFound { tag: String },
    #[error("group `{tag}` is not a selector group")]
    SelectorGroupTypeMismatch { tag: String },
    #[error("selector group `{group_tag}` does not contain outbound `{outbound_tag}`")]
    SelectorOutboundNotFound {
        group_tag: String,
        outbound_tag: String,
    },
}
