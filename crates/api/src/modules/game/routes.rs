use axum::Router;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use lucyd::lucy_http;

use crate::errors::ApiError;
use crate::state::AppState;

use super::dto::{GameStateResponse, StartGameRequest};
use super::service::GameService;

pub fn router() -> Router<AppState> {
    use axum::routing::{get, post};
    Router::new()
        .route("/api/v1/game/start", post(start_game))
        .route("/api/v1/game/state", get(game_state))
        .route("/api/v1/game/end", post(end_game))
}

#[lucy_http(
    method      = "POST",
    path        = "/api/v1/game/start",
    tags        = "game",
    request     = StartGameRequest,
    response    = GameStateResponse,
    description = "Start a new game session with the chosen character",
)]
pub async fn start_game(
    State(state): State<AppState>,
    axum::Json(body): axum::Json<StartGameRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let snapshot = GameService::new(&state)
        .start(body.player_id, body.character_id)
        .await?;

    Ok((
        StatusCode::OK,
        axum::Json(GameStateResponse::from(snapshot)),
    ))
}

#[lucy_http(
    method      = "GET",
    path        = "/api/v1/game/state",
    tags        = "game",
    response    = GameStateResponse,
    description = "Returns the current game engine state, or 404 if no game is running",
)]
pub async fn game_state(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    // Lock order: engine FIRST [§ 4.4]
    let engine_guard = state.game_engine.lock().await;

    let Some(engine) = engine_guard.as_ref() else {
        drop(engine_guard);
        return Err(ApiError::NotFound("no_game_in_progress".to_owned()));
    };

    let snapshot = engine.take_snapshot();
    drop(engine_guard);

    Ok((
        StatusCode::OK,
        axum::Json(GameStateResponse::from(snapshot)),
    ))
}

#[lucy_http(
    method      = "POST",
    path        = "/api/v1/game/end",
    tags        = "game",
    response    = GameStateResponse,
    description = "Force-end the current game and persist the final score",
)]
pub async fn end_game(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    let snapshot = GameService::new(&state).end().await?;

    Ok((
        StatusCode::OK,
        axum::Json(GameStateResponse::from(snapshot)),
    ))
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    use super::*;

    async fn test_state() -> AppState {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        AppState::new(b"flipper-dev-secret-change-in-prod".to_vec(), pool)
    }

    fn start_body(player_id: &str, character_id: u8) -> Body {
        Body::from(
            serde_json::to_vec(&serde_json::json!({
                "player_id": player_id,
                "character_id": character_id
            }))
            .unwrap(),
        )
    }

    async fn post(app: axum::Router, path: &str, body: Body) -> (StatusCode, serde_json::Value) {
        let resp = app
            .oneshot(
                Request::post(path)
                    .header("content-type", "application/json")
                    .body(body)
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = resp.status();
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
        (status, json)
    }

    async fn get(app: axum::Router, path: &str) -> (StatusCode, serde_json::Value) {
        let resp = app
            .oneshot(Request::get(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = resp.status();
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
        (status, json)
    }

    #[tokio::test]
    async fn start_game_returns_200_with_valid_request() {
        let state = test_state().await;
        let app = router().with_state(state);

        let (status, body) = post(app, "/api/v1/game/start", start_body("alice", 1)).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["phase"], "in_game");
    }

    #[tokio::test]
    async fn start_game_returns_409_when_game_already_active() {
        let state = test_state().await;
        let app = router().with_state(state);

        let (s1, _) = post(app.clone(), "/api/v1/game/start", start_body("bob", 1)).await;
        assert_eq!(s1, StatusCode::OK);

        let (s2, body) = post(app, "/api/v1/game/start", start_body("bob", 1)).await;
        assert_eq!(s2, StatusCode::CONFLICT);
        assert_eq!(body["error"], "conflict");
    }

    #[tokio::test]
    async fn game_state_returns_404_when_no_game() {
        let state = test_state().await;
        let app = router().with_state(state);

        let (status, body) = get(app, "/api/v1/game/state").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"], "not_found");
    }

    #[tokio::test]
    async fn game_state_returns_200_when_game_active() {
        let state = test_state().await;
        let app = router().with_state(state);

        let (s, _) = post(app.clone(), "/api/v1/game/start", start_body("carol", 2)).await;
        assert_eq!(s, StatusCode::OK);

        let (status, body) = get(app, "/api/v1/game/state").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["phase"], "in_game");
    }

    #[tokio::test]
    async fn end_game_returns_404_when_no_game() {
        let state = test_state().await;
        let app = router().with_state(state);

        let (status, body) = post(app, "/api/v1/game/end", Body::empty()).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"], "not_found");
    }

    #[tokio::test]
    async fn end_game_returns_200_and_clears_game() {
        let state = test_state().await;
        let app = router().with_state(state);

        let (s, _) = post(app.clone(), "/api/v1/game/start", start_body("dave", 1)).await;
        assert_eq!(s, StatusCode::OK);

        let (s_end, body_end) = post(app.clone(), "/api/v1/game/end", Body::empty()).await;
        assert_eq!(s_end, StatusCode::OK);
        assert_eq!(body_end["phase"], "game_over");

        // Engine is cleared — next GET /state must return 404
        let (s_state, _) = get(app, "/api/v1/game/state").await;
        assert_eq!(s_state, StatusCode::NOT_FOUND);
    }
}
