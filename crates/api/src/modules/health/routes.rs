use axum::Router;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use lucyd::lucy_http;
use serde::Serialize;
use utoipa::ToSchema;

use crate::state::AppState;

#[derive(Serialize, ToSchema)]
pub struct HealthResponse {
    status: &'static str,
}

pub fn router() -> Router<AppState> {
    Router::new().route("/health", get(health_check))
}

#[lucy_http(method = "GET", path = "/health", tags = "system", description = "Returns 200 OK when the service is up")]
pub async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, axum::Json(HealthResponse { status: "ok" }))
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    use super::*;

    #[tokio::test]
    async fn health_returns_ok() {
        let app = router().with_state(AppState::new(b"flipper-dev-secret-change-in-prod".to_vec()));

        let response = app
            .oneshot(Request::get("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}
