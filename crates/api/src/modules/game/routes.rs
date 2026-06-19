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
        .route("/api/v1/characters", get(get_characters))
}

#[lucy_http(
    method      = "POST",
    path        = "/api/v1/game/start",
    tags        = "game",
    request     = StartGameRequest,
    response    = GameStateResponse,
    description = "Start a new game session with the chosen character (slug: enforcer | viper | ghost | oracle)",
)]
pub async fn start_game(
    State(state): State<AppState>,
    axum::Json(body): axum::Json<StartGameRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let snapshot = GameService::new(&state).start(body.character).await?;

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

/// Returns the gameplay roster (no visual data — the front merges by slug with its own config).
pub async fn get_characters() -> impl IntoResponse {
    let characters = serde_json::json!([
        {
            "id": "enforcer",
            "ulti_id": "multiball_split",
            "shape": "instant",
            "cancellable": false,
            "charge_max": 320,
            "charge_profile": {
                "weight_bumper": 1.0,
                "weight_rail": 0.3,
                "weight_combo": 1.0,
                "weight_other": 1.0,
                "time_rate": 0.0
            }
        },
        {
            "id": "viper",
            "ulti_id": "rampage",
            "shape": "sustained",
            "cancellable": false,
            "duration_ms": 8000,
            "charge_max": 360,
            "payload": { "multiplier": 5.0 },
            "charge_profile": {
                "weight_bumper": 1.0,
                "weight_rail": 1.0,
                "weight_combo": 1.0,
                "weight_other": 1.0,
                "time_rate": 0.0
            }
        },
        {
            "id": "ghost",
            "ulti_id": "mimic",
            "shape": "inherited",
            "charge_max": 300,
            "charge_profile": {
                "weight_bumper": 1.0,
                "weight_rail": 1.0,
                "weight_combo": 1.0,
                "weight_other": 1.0,
                "time_rate": 0.0
            }
        },
        {
            "id": "oracle",
            "ulti_id": "time_slow",
            "shape": "sustained",
            "cancellable": true,
            "duration_ms": 5000,
            "charge_max": 240,
            "payload": { "slow_factor": 0.25 },
            "charge_profile": {
                "weight_bumper": 1.0,
                "weight_rail": 1.0,
                "weight_combo": 1.0,
                "weight_other": 1.0,
                "time_rate": 1.0
            }
        }
    ]);
    (StatusCode::OK, axum::Json(characters))
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

    fn start_body(character: &str) -> Body {
        Body::from(serde_json::to_vec(&serde_json::json!({ "character": character })).unwrap())
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

        let (status, body) = post(app, "/api/v1/game/start", start_body("enforcer")).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["phase"], "in_game");
    }

    #[tokio::test]
    async fn start_game_returns_409_when_game_already_active() {
        let state = test_state().await;
        let app = router().with_state(state);

        let (s1, _) = post(app.clone(), "/api/v1/game/start", start_body("viper")).await;
        assert_eq!(s1, StatusCode::OK);

        let (s2, body) = post(app, "/api/v1/game/start", start_body("viper")).await;
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

        let (s, _) = post(app.clone(), "/api/v1/game/start", start_body("ghost")).await;
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

        let (s, _) = post(app.clone(), "/api/v1/game/start", start_body("oracle")).await;
        assert_eq!(s, StatusCode::OK);

        let (s_end, body_end) = post(app.clone(), "/api/v1/game/end", Body::empty()).await;
        assert_eq!(s_end, StatusCode::OK);
        assert_eq!(body_end["phase"], "game_over");

        let (s_state, _) = get(app, "/api/v1/game/state").await;
        assert_eq!(s_state, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn get_characters_returns_4_entries() {
        let state = test_state().await;
        let app = router().with_state(state);

        let (status, body) = get(app, "/api/v1/characters").await;
        assert_eq!(status, StatusCode::OK);
        let arr = body.as_array().expect("should be a JSON array");
        assert_eq!(arr.len(), 4);
    }
}
