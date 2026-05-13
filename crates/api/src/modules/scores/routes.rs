use axum::Router;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use lucyd::lucy_http;

use crate::errors::ApiError;
use crate::state::AppState;

use super::dto::{LeaderboardResponse, SaveScoreRequest};
use super::service;

pub fn router() -> Router<AppState> {
    use axum::routing::get;
    Router::new()
        .route("/api/v1/scores", get(leaderboard).post(save_score))
        .route("/api/v1/scores/{player_id}", get(player_scores))
}

#[lucy_http(
    method      = "GET",
    path        = "/api/v1/scores",
    tags        = "scores",
    response    = LeaderboardResponse,
    description = "Top 10 leaderboard sorted by score",
)]
pub async fn leaderboard(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    let scores = service::get_leaderboard(&state.db_pool, 10).await?;
    Ok((StatusCode::OK, axum::Json(LeaderboardResponse { scores })))
}

#[lucy_http(
    method      = "GET",
    path        = "/api/v1/scores/{player_id}",
    tags        = "scores",
    response    = LeaderboardResponse,
    description = "Score history for a specific player",
)]
pub async fn player_scores(
    Path(player_id): Path<String>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let scores = service::get_player_scores(&state.db_pool, &player_id).await?;
    Ok((StatusCode::OK, axum::Json(LeaderboardResponse { scores })))
}

#[lucy_http(
    method      = "POST",
    path        = "/api/v1/scores",
    tags        = "scores",
    request     = SaveScoreRequest,
    response    = LeaderboardResponse,
    description = "Manually save a score (debug use)",
)]
pub async fn save_score(
    State(state): State<AppState>,
    axum::Json(req): axum::Json<SaveScoreRequest>,
) -> Result<impl IntoResponse, ApiError> {
    service::save_score(&state.db_pool, req).await?;
    Ok(StatusCode::CREATED)
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

    async fn post(app: axum::Router, path: &str, body: serde_json::Value) -> (StatusCode, serde_json::Value) {
        let resp = app
            .oneshot(
                Request::post(path)
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
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
    async fn leaderboard_returns_empty_initially() {
        let app = router().with_state(test_state().await);

        let (status, body) = get(app, "/api/v1/scores").await;

        assert_eq!(status, StatusCode::OK);
        let scores = body["scores"].as_array().unwrap();
        assert!(scores.is_empty());
    }

    #[tokio::test]
    async fn save_score_returns_201() {
        let app = router().with_state(test_state().await);

        let (status, _) = post(
            app,
            "/api/v1/scores",
            serde_json::json!({
                "player_id": "alice",
                "character_id": 1,
                "score": 5000,
                "boss_reached": 1
            }),
        )
        .await;

        assert_eq!(status, StatusCode::CREATED);
    }

    #[tokio::test]
    async fn leaderboard_shows_saved_score() {
        let state = test_state().await;
        let app = router().with_state(state);

        let (s, _) = post(
            app.clone(),
            "/api/v1/scores",
            serde_json::json!({
                "player_id": "bob",
                "character_id": 2,
                "score": 7500,
                "boss_reached": 0
            }),
        )
        .await;
        assert_eq!(s, StatusCode::CREATED);

        let (status, body) = get(app, "/api/v1/scores").await;
        assert_eq!(status, StatusCode::OK);
        let scores = body["scores"].as_array().unwrap();
        assert_eq!(scores.len(), 1);
        assert_eq!(scores[0]["player_id"], "bob");
        assert_eq!(scores[0]["score"], 7500);
    }

    #[tokio::test]
    async fn player_scores_returns_empty_for_unknown_player() {
        let app = router().with_state(test_state().await);

        let (status, body) = get(app, "/api/v1/scores/unknown_player").await;

        assert_eq!(status, StatusCode::OK);
        let scores = body["scores"].as_array().unwrap();
        assert!(scores.is_empty());
    }

    #[tokio::test]
    async fn player_scores_returns_history_after_save() {
        let state = test_state().await;
        let app = router().with_state(state);

        let (s, _) = post(
            app.clone(),
            "/api/v1/scores",
            serde_json::json!({
                "player_id": "carol",
                "character_id": 3,
                "score": 12000,
                "boss_reached": 2
            }),
        )
        .await;
        assert_eq!(s, StatusCode::CREATED);

        let (status, body) = get(app, "/api/v1/scores/carol").await;
        assert_eq!(status, StatusCode::OK);
        let scores = body["scores"].as_array().unwrap();
        assert_eq!(scores.len(), 1);
        assert_eq!(scores[0]["player_id"], "carol");
        assert_eq!(scores[0]["score"], 12000);
        assert_eq!(scores[0]["boss_reached"], 2);
    }

    #[tokio::test]
    async fn save_score_with_invalid_body_returns_422() {
        let app = router().with_state(test_state().await);

        let (status, _) = post(
            app,
            "/api/v1/scores",
            serde_json::json!({ "garbage": true }),
        )
        .await;

        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }
}
