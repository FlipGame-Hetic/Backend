use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use thiserror::Error;

// Central API error type.
// Used by handler modules (auth, room, realtime) to return typed HTTP errors.
#[derive(Debug, Error)]
pub enum ApiError {
    #[error("Bad request: {0}")]
    BadRequest(String),

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
            ApiError::BadRequest(_) => (StatusCode::BAD_REQUEST, "bad_request"),
            ApiError::Serialization(_) => (StatusCode::BAD_REQUEST, "serialization_error"),
        };

        let body = ErrorBody {
            error: error_type.to_owned(),
            message: self.to_string(),
        };

        (status, axum::Json(body)).into_response()
    }
}
