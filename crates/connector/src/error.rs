use thiserror::Error;

pub type ConnectorResult<T> = Result<T, ConnectorError>;

#[derive(Debug, Error)]
pub enum ConnectorError {
    #[error("connector feature `{feature}` is disabled for `{sink_type}` event sink `{tag}`")]
    FeatureDisabled {
        feature: &'static str,
        sink_type: &'static str,
        tag: String,
    },
    #[error("failed to read api key from environment variable `{name}`: {source}")]
    ReadApiKeyEnv {
        name: String,
        #[source]
        source: std::env::VarError,
    },
    #[error("api key environment variable `{name}` must not be empty")]
    EmptyApiKeyEnv { name: String },
    #[error("failed to open jsonl event sink `{tag}` at `{path}`: {source}")]
    OpenJsonLineSink {
        tag: String,
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("event dispatcher failed to start")]
    DispatcherStart,
    #[error("api error while building connector: {0}")]
    Api(#[from] zero_api::ApiError),
}
