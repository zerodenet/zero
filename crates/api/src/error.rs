use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::Permission;

#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
#[error("{code}: {message}")]
pub struct ApiError {
    pub code: ApiErrorCode,
    pub message: String,
    pub field_path: Option<String>,
    pub cause: Option<String>,
}

impl ApiError {
    pub fn new(code: ApiErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            field_path: None,
            cause: None,
        }
    }

    pub fn permission_denied(required: Permission) -> Self {
        Self::new(
            ApiErrorCode::PermissionDenied,
            format!("permission `{required:?}` is required"),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ApiErrorCode {
    #[error("not-found")]
    NotFound,
    #[error("invalid-argument")]
    InvalidArgument,
    #[error("permission-denied")]
    PermissionDenied,
    #[error("feature-disabled")]
    FeatureDisabled,
    #[error("conflict")]
    Conflict,
    #[error("unsupported")]
    Unsupported,
    #[error("internal")]
    Internal,
}

impl ApiErrorCode {
    /// Stable string code used in JSON error responses.
    pub fn as_code_str(&self) -> &'static str {
        match self {
            Self::NotFound => "not_found",
            Self::InvalidArgument => "invalid_argument",
            Self::PermissionDenied => "permission_denied",
            Self::FeatureDisabled => "feature_disabled",
            Self::Conflict => "conflict",
            Self::Unsupported => "unsupported",
            Self::Internal => "internal",
        }
    }
}
