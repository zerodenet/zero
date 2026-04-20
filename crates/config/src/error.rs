use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read config `{path}`: {source}")]
    ReadConfig {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse config: {0}")]
    ParseConfig(#[from] serde_json::Error),
    #[error("`{scope}` tag must not be empty")]
    EmptyTag { scope: &'static str },
    #[error("duplicate `{scope}` tag `{tag}`")]
    DuplicateTag { scope: &'static str, tag: String },
    #[error(
        "duplicate inbound listen endpoint `{address}:{port}`; use `mixed` for multi-protocol same-port listening"
    )]
    DuplicateInboundListen { address: String, port: u16 },
    #[error("invalid rule condition: {0}")]
    InvalidRuleCondition(String),
    #[error("invalid route action: {0}")]
    InvalidRouteAction(String),
    #[error("route references undefined outbound tag `{tag}`")]
    UndefinedOutboundTag { tag: String },
}
