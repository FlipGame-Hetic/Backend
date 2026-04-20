use axum::Router;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use lucyd::lucy_http;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use shared::screen::{ScreenEnvelope, ScreenId};
use tracing::{debug, warn};
use utoipa::ToSchema;

use crate::state::AppState;

#[derive(Debug, Serialize, Deserialize, ToSchema, JsonSchema)]
pub struct ConnectedScreensResponse {
    pub screens: Vec<ScreenId>,
    pub count: usize,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, JsonSchema)]
pub struct SendResponse {
    pub delivered: usize,
    pub missed: Vec<ScreenId>,
    pub intercepted: bool,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/screens/connected", get(connected_screens))
        .route("/api/v1/screens/send", post(send_to_screen))
}

#[utoipa::path(
    get,
    path = "/api/v1/screens/connected",
    tag = "screens",
    responses(
        (status = 200, description = "Connected screens", body = ConnectedScreensResponse),
    )
)]
#[lucy_http(
    method      = "GET",
    path        = "/api/v1/screens/connected",
    tags        = "screens",
    response    = ConnectedScreensResponse,
    description = "List all currently connected screens",
)]
pub async fn connected_screens(State(state): State<AppState>) -> impl IntoResponse {
    let screens = state.screen_registry.connected_screens().await;
    let count = screens.len();
    debug!(count, "listing connected screens");
    (
        StatusCode::OK,
        axum::Json(ConnectedScreensResponse { screens, count }),
    )
}

/// Debug endpoint — inject a `ScreenEnvelope` without a real WS connection.
#[utoipa::path(
    post,
    path = "/api/v1/screens/send",
    tag = "screens",
    request_body(
        content = ScreenEnvelope,
        content_type = "application/json",
        example = json!({
            "from": "front_screen",
            "to": { "kind": "screen", "id": "back_screen" },
            "event_type": "game_state_update",
            "payload": { "score": 42000, "combo": 3 }
        })
    ),
    responses(
        (status = 200, description = "Dispatched", body = SendResponse),
        (status = 422, description = "Invalid envelope"),
    )
)]
#[lucy_http(
    method      = "POST",
    path        = "/api/v1/screens/send",
    tags        = "screens",
    request     = ScreenEnvelope,
    response    = SendResponse,
    description = "Inject a ScreenEnvelope for debug dispatch without a real WS connection",
)]
pub async fn send_to_screen(
    State(state): State<AppState>,
    axum::Json(envelope): axum::Json<ScreenEnvelope>,
) -> impl IntoResponse {
    debug!(from = %envelope.from, to = ?envelope.to, event_type = %envelope.event_type, "dispatching envelope");

    let result = state.screen_router.dispatch(envelope).await;

    if !result.missed.is_empty() {
        warn!(missed = ?result.missed, "target screens not connected");
    }

    (
        StatusCode::OK,
        axum::Json(SendResponse {
            delivered: result.delivered,
            missed: result.missed,
            intercepted: result.intercepted,
        }),
    )
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    use super::*;

    fn test_state() -> AppState {
        AppState::new(b"flipper-dev-secret-change-in-prod".to_vec())
    }

    #[tokio::test]
    async fn connected_screens_returns_empty_initially() {
        let app = router().with_state(test_state());

        let resp = app
            .oneshot(
                Request::get("/api/v1/screens/connected")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let data: ConnectedScreensResponse = serde_json::from_slice(&body).unwrap();

        assert_eq!(data.count, 0);
        assert!(data.screens.is_empty());
    }

    #[tokio::test]
    async fn send_to_disconnected_screen_returns_missed() {
        let app = router().with_state(test_state());

        let envelope = serde_json::json!({
            "from": "front_screen",
            "to": { "kind": "screen", "id": "back_screen" },
            "event_type": "test_event",
            "payload": { "hello": "world" }
        });

        let resp = app
            .oneshot(
                Request::post("/api/v1/screens/send")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&envelope).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let data: SendResponse = serde_json::from_slice(&body).unwrap();

        assert_eq!(data.delivered, 0);
        assert_eq!(data.missed, vec![ScreenId::BackScreen]);
        assert!(!data.intercepted);
    }

    #[tokio::test]
    async fn send_with_invalid_body_returns_422() {
        let app = router().with_state(test_state());

        let resp = app
            .oneshot(
                Request::post("/api/v1/screens/send")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"garbage": true}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }
}
