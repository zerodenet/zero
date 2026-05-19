use serde::Serialize;
use zero_api::ApiError;

/// Unified API response envelope.
#[derive(Debug, Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub api_version: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ApiErrorBody>,
}

#[derive(Debug, Serialize)]
pub struct ApiErrorBody {
    pub code: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn ok(result: T) -> Self {
        Self {
            api_version: zero_api::API_VERSION,
            request_id: None,
            ok: true,
            result: Some(result),
            error: None,
        }
    }

    #[allow(dead_code)]
    pub fn ok_with_id(request_id: Option<String>, result: T) -> Self {
        Self {
            api_version: zero_api::API_VERSION,
            request_id,
            ok: true,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(error: &ApiError) -> Self {
        Self {
            api_version: zero_api::API_VERSION,
            request_id: None,
            ok: false,
            result: None,
            error: Some(ApiErrorBody {
                code: error.code.as_code_str(),
                message: error.message.clone(),
                field_path: error.field_path.clone(),
                details: None,
            }),
        }
    }
}

/// Map an `ApiError` to the appropriate HTTP status code.
pub fn api_error_status(error: &ApiError) -> &'static str {
    match error.code {
        zero_api::ApiErrorCode::NotFound => "HTTP/1.1 404 Not Found\r\n",
        zero_api::ApiErrorCode::InvalidArgument => "HTTP/1.1 400 Bad Request\r\n",
        zero_api::ApiErrorCode::PermissionDenied => "HTTP/1.1 403 Forbidden\r\n",
        zero_api::ApiErrorCode::FeatureDisabled | zero_api::ApiErrorCode::Unsupported => {
            "HTTP/1.1 501 Not Implemented\r\n"
        }
        zero_api::ApiErrorCode::Conflict => "HTTP/1.1 409 Conflict\r\n",
        zero_api::ApiErrorCode::Internal => "HTTP/1.1 500 Internal Server Error\r\n",
    }
}
