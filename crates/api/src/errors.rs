use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use thiserror::Error;

/// Domain error type returned by all route handlers
///
/// Each variant maps to a specific HTTP status code via `IntoResponse`
#[derive(Debug, Error)]
pub enum ApiError {
    #[error("Bad request: {0}")]
    BadRequest(String), // 400

    #[error("Not found: {0}")]
    NotFound(String), // 404

    #[error("Conflict: {0}")]
    Conflict(String), // 409

    #[error("Unauthorized: {0}")]
    Unauthorized(String), // 401

    #[error("Internal error: {0}")]
    Internal(String), // 500

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error), // 400
}

// All sqlx errors bubble up as 500 — callers never need to match on DB variants
impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        Self::Internal(e.to_string())
    }
}

/// JSON body shape returned for every error response: `{ "error": "<code>", "message": "<detail>" }`
#[derive(Debug, Serialize)]
struct ErrorBody {
    error: String,
    message: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error_type) = match &self {
            ApiError::BadRequest(_) => (StatusCode::BAD_REQUEST, "bad_request"),
            ApiError::NotFound(_) => (StatusCode::NOT_FOUND, "not_found"),
            ApiError::Conflict(_) => (StatusCode::CONFLICT, "conflict"),
            ApiError::Unauthorized(_) => (StatusCode::UNAUTHORIZED, "unauthorized"),
            ApiError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal_error"),
            ApiError::Serialization(_) => (StatusCode::BAD_REQUEST, "serialization_error"),
        };

        let body = ErrorBody {
            error: error_type.to_owned(),
            message: self.to_string(),
        };

        (status, axum::Json(body)).into_response()
    }
}
