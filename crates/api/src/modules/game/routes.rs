use axum::Router;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use game_logic::GameEngine;
use lucyd::lucy_http;
use tracing::warn;

use crate::errors::ApiError;
use crate::modules::realtime::bridge_sync::sync_game_state_to_bridge;
use crate::modules::scores::dto::SaveScoreRequest;
use crate::modules::scores::service as score_service;
use crate::state::{AppState, GameSession};

use super::dto::{GameStateResponse, StartGameRequest};

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
    // Lock order: engine FIRST, session SECOND [§ 4.4]
    let mut engine_guard = state.game_engine.lock().await;
    let mut session_guard = state.active_session.lock().await;

    // Policy: refuse if a game is already in progress [§ 4.2]
    if session_guard.is_some() {
        drop(session_guard);
        drop(engine_guard);
        return Err(ApiError::Conflict("game_already_in_progress".to_owned()));
    }

    let mut engine = GameEngine::new(body.character_id);
    let envelopes = engine.process(game_logic::GameEvent::StartGame {
        player_id: body.player_id.clone(),
    });

    let state_snapshot = engine.state.clone();

    *engine_guard = Some(engine);
    *session_guard = Some(GameSession {
        player_id: body.player_id,
        character_id: body.character_id,
        boss_reached: 0,
    });

    // Unlock before any await [§ 4.4]
    drop(session_guard);
    drop(engine_guard);

    let device_id = state.active_device_id.read().await.clone();
    if device_id.is_none() {
        warn!("no bridge connected — ESP32 sync skipped");
    }

    for env in envelopes {
        let _ = state.screen_router.dispatch(env).await;
    }

    if let Some(id) = device_id {
        sync_game_state_to_bridge(&state_snapshot, &state.hub, &id);
    }

    Ok((
        StatusCode::OK,
        axum::Json(GameStateResponse::from(state_snapshot)),
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

    let snapshot = engine.state.clone();
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
    // Lock order: engine FIRST, session SECOND [§ 4.4]
    let mut engine_guard = state.game_engine.lock().await;
    let mut session_guard = state.active_session.lock().await;

    let Some(engine) = engine_guard.as_mut() else {
        drop(session_guard);
        drop(engine_guard);
        return Err(ApiError::NotFound("no_game_in_progress".to_owned()));
    };

    let envelopes = engine.process(game_logic::GameEvent::EndGame);
    let score_snapshot = engine.state.score;
    let state_snapshot = engine.state.clone();
    let session_snapshot = session_guard.take();

    *engine_guard = None;

    // Unlock before any await [§ 4.4]
    drop(session_guard);
    drop(engine_guard);

    for env in envelopes {
        let _ = state.screen_router.dispatch(env).await;
    }

    if let Some(session) = session_snapshot {
        let req = SaveScoreRequest {
            player_id: session.player_id,
            character_id: session.character_id,
            score: score_snapshot,
            boss_reached: session.boss_reached,
        };
        score_service::save_score(&state.db_pool, req).await?;
    }

    Ok((
        StatusCode::OK,
        axum::Json(GameStateResponse::from(state_snapshot)),
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

        let (s1, _) = post(
            app.clone(),
            "/api/v1/game/start",
            start_body("bob", 1),
        )
        .await;
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

        let (s, _) = post(
            app.clone(),
            "/api/v1/game/start",
            start_body("carol", 2),
        )
        .await;
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

        let (s, _) = post(
            app.clone(),
            "/api/v1/game/start",
            start_body("dave", 1),
        )
        .await;
        assert_eq!(s, StatusCode::OK);

        let (s_end, body_end) = post(
            app.clone(),
            "/api/v1/game/end",
            Body::empty(),
        )
        .await;
        assert_eq!(s_end, StatusCode::OK);
        assert_eq!(body_end["phase"], "game_over");

        // Engine is cleared — next GET /state must return 404
        let (s_state, _) = get(app, "/api/v1/game/state").await;
        assert_eq!(s_state, StatusCode::NOT_FOUND);
    }
}
