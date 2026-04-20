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
    #[error("route references outbound tag `{tag}` but no such outbound exists")]
    MissingOutbound { tag: String },
    #[error("inbound task exited unexpectedly")]
    InboundTaskExited,
}
