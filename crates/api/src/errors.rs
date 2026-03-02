use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use thiserror::Error;
use tracing::error;

// Central API error type.
// Used by handler modules (auth, room, realtime) to return typed HTTP errors.
// Variants will be populated as modules are implemented.
#[derive(Debug, Error)]
pub enum ApiError {
    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Websocket error: {0}")]
    WebSocket(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

#[derive(Debug, Serialize)]
struct ErrorBody {
    error: String,
    message: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error_type) = match &self {
            ApiError::NotFound(_) => (StatusCode::NOT_FOUND, "not_found"),
            ApiError::BadRequest(_) => (StatusCode::BAD_REQUEST, "bad_request"),
            ApiError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal_error"),
            ApiError::WebSocket(_) => (StatusCode::INTERNAL_SERVER_ERROR, "websocket_error"),
            ApiError::Serialization(_) => (StatusCode::BAD_REQUEST, "serialization_error"),
        };

        if matches!(status, StatusCode::INTERNAL_SERVER_ERROR) {
            error!(error = %self, "Internal server error");
        }

        let body = ErrorBody {
            error: error_type.to_owned(),
            message: self.to_string(),
        };

        (status, axum::Json(body)).into_response()
    }
}

// Convenience alias for handler return types.
pub type ApiResult<T> = Result<T, ApiError>;