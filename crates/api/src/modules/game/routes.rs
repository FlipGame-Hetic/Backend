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
    let character = body.character.clone();
    let snapshot = GameService::new(&state).start(body.character).await?;
    let mut response = GameStateResponse::from(snapshot);
    response.character = Some(character);

    Ok((StatusCode::OK, axum::Json(response)))
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
    // Release the engine lock before acquiring active_session to respect lock order
    drop(engine_guard);

    let character = state
        .active_session
        .lock()
        .await
        .as_ref()
        .map(|s| s.character_slug.clone());

    let mut response = GameStateResponse::from(snapshot);
    response.character = character;

    Ok((StatusCode::OK, axum::Json(response)))
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

/// Returns the full character roster with their live game-engine parameters.
///
/// Values are read directly from [`game_logic`] structs and the runtime [`GameConfig`],
/// so they always reflect the current admin configuration patching the config via
/// `PATCH /api/v1/admin/config` is immediately visible here without a restart.
///
/// Visual assets, display names, and lore copy live in the frontend config; this
/// endpoint is authoritative only for the fields the game engine actually uses
/// (ult shape, charge weights, payload scalars, etc.).
pub async fn get_characters() -> impl IntoResponse {
    use game_logic::{UltiShape, select_character};

    // Acquire the config read-guard once for the whole iterator instead of once per
    // character `stats()` also calls `config::get()` internally but releases it
    // immediately; holding this guard concurrently is safe because it is a RwLock.
    let cfg = game_logic::engine::config::get();

    let characters: Vec<serde_json::Value> = ["enforcer", "viper", "ghost", "oracle"]
        .iter()
        .map(|&slug| {
            // `select_character` returns a boxed trait object; it logs a warning and
            // falls back to Enforcer on unknown slugs, so this list must stay in sync
            // with the match arms in `game_logic::player::personnages::character`.
            let c = select_character(slug);

            // `stats()` reads the live config internally, so charge_max and all weights
            // already reflect any patch applied since the process started.
            let profile = c.stats().charge_profile;

            // Start with the fields that are common across all characters, then
            // add shape-specific and character-specific keys below
            let mut obj = serde_json::json!({
                "id": c.slug(),
                "ulti_id": c.ulti_id(),
                "charge_max": profile.charge_max,
                "charge_profile": {
                    "weight_bumper": profile.weight_bumper,
                    "weight_rail":   profile.weight_rail,
                    "weight_combo":  profile.weight_combo,
                    "weight_other":  profile.weight_other,
                    // Non-zero only for Oracle: passive charge that ticks up over time
                    // regardless of ball events.
                    "time_rate": profile.time_rate,
                }
            });

            // `UltiShape` drives how the frontend arms and fires the ultimate:
            //   - Instant    fires immediately, no hold logic needed on the client side.
            //   - Sustained  runs for `duration_ms` and may be cancelled mid-flight.
            //   - Inherited  takes the shape of the current game cycle; the client must
            //                resolve the concrete shape at activation time.
            match c.ulti_shape() {
                UltiShape::Instant => {
                    obj["shape"] = serde_json::json!("instant");
                    obj["cancellable"] = serde_json::json!(false);
                }
                UltiShape::Sustained {
                    duration_ms,
                    cancellable,
                } => {
                    obj["shape"] = serde_json::json!("sustained");
                    obj["cancellable"] = serde_json::json!(cancellable);
                    obj["duration_ms"] = serde_json::json!(duration_ms);
                }
                // Ghost: no `cancellable` or `duration_ms` — the client reads the active
                // cycle's shape when the ult actually triggers.
                UltiShape::Inherited => {
                    obj["shape"] = serde_json::json!("inherited");
                }
            }

            // `payload` carries ult-specific scalars that the frontend uses for its own
            // animations or HUD display. Only characters whose ult has a tunable scalar
            // expose this field; others omit it entirely to keep the contract minimal.
            match slug {
                "viper" => {
                    // Score multiplier applied to all events during the Rampage window.
                    obj["payload"] =
                        serde_json::json!({ "multiplier": cfg.viper_rampage_multiplier });
                }
                "oracle" => {
                    // Fraction of normal speed (0 < slow_factor < 1) during Time Slow.
                    obj["payload"] = serde_json::json!({ "slow_factor": cfg.oracle_slow_factor });
                }
                _ => {}
            }

            obj
        })
        .collect();

    (
        StatusCode::OK,
        axum::Json(serde_json::Value::Array(characters)),
    )
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
        assert_eq!(body["character"], "ghost");
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
