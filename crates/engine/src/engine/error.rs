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
}
