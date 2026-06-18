use axum::Router;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use lucyd::lucy_http;

use crate::errors::ApiError;
use crate::state::AppState;

use super::dto::{LeaderboardResponse, SaveScoreRequest};
use super::service;

pub fn router() -> Router<AppState> {
    use axum::routing::get;
    Router::new().route("/api/v1/scores", get(leaderboard).post(save_score))
}

#[lucy_http(
    method      = "GET",
    path        = "/api/v1/scores",
    tags        = "scores",
    response    = LeaderboardResponse,
    description = "Returns the top-10 all-time high scores sorted by score descending. \
                   This is the same leaderboard pushed to back_screen via LeaderboardUpdate \
                   at the end of every game.",
)]
pub async fn leaderboard(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    let scores = service::get_leaderboard(&state.db_pool, 10).await?;
    Ok((StatusCode::OK, axum::Json(LeaderboardResponse { scores })))
}

#[lucy_http(
    method      = "POST",
    path        = "/api/v1/scores",
    tags        = "scores",
    request     = SaveScoreRequest,
    description = "Debug endpoint: attempt to insert a score into the top-10 leaderboard. \
                   Returns 201 when the score qualifies and was persisted, \
                   200 when it does not beat the current minimum and is discarded.",
)]
pub async fn save_score(
    State(state): State<AppState>,
    axum::Json(req): axum::Json<SaveScoreRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let saved = service::save_score(&state.db_pool, req).await?;
    if saved {
        Ok(StatusCode::CREATED)
    } else {
        Ok(StatusCode::OK)
    }
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

    async fn post(
        app: axum::Router,
        path: &str,
        body: serde_json::Value,
    ) -> (StatusCode, serde_json::Value) {
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
        assert!(body["scores"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn save_score_returns_201() {
        let app = router().with_state(test_state().await);

        let (status, _) = post(
            app,
            "/api/v1/scores",
            serde_json::json!({ "character_id": 1, "score": 5000, "boss_reached": 1 }),
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
            serde_json::json!({ "character_id": 2, "score": 7500, "boss_reached": 0 }),
        )
        .await;
        assert_eq!(s, StatusCode::CREATED);

        let (status, body) = get(app, "/api/v1/scores").await;
        assert_eq!(status, StatusCode::OK);
        let scores = body["scores"].as_array().unwrap();
        assert_eq!(scores.len(), 1);
        assert_eq!(scores[0]["score"], 7500);
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

    async fn fill_leaderboard(app: axum::Router, base_score: u64) {
        for i in 0..10u64 {
            post(
                app.clone(),
                "/api/v1/scores",
                serde_json::json!({
                    "character_id": 1,
                    "score": base_score + i * 100,
                    "boss_reached": 0
                }),
            )
            .await;
        }
    }

    #[tokio::test]
    async fn leaderboard_never_exceeds_10_entries() {
        let state = test_state().await;
        let app = router().with_state(state);

        fill_leaderboard(app.clone(), 1000).await;

        for i in 0..3u64 {
            post(
                app.clone(),
                "/api/v1/scores",
                serde_json::json!({ "character_id": 1, "score": 9999 + i, "boss_reached": 0 }),
            )
            .await;
        }

        let (status, body) = get(app, "/api/v1/scores").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["scores"].as_array().unwrap().len(), 10);
    }

    #[tokio::test]
    async fn score_below_minimum_does_not_enter_full_leaderboard() {
        let state = test_state().await;
        let app = router().with_state(state);

        fill_leaderboard(app.clone(), 1000).await;

        let (status, _) = post(
            app.clone(),
            "/api/v1/scores",
            serde_json::json!({ "character_id": 1, "score": 500, "boss_reached": 0 }),
        )
        .await;

        assert_eq!(status, StatusCode::OK);

        let (_, body) = get(app, "/api/v1/scores").await;
        assert_eq!(body["scores"].as_array().unwrap().len(), 10);
    }

    #[tokio::test]
    async fn score_equal_to_minimum_does_not_enter_full_leaderboard() {
        let state = test_state().await;
        let app = router().with_state(state);

        fill_leaderboard(app.clone(), 1000).await;

        let (status, _) = post(
            app.clone(),
            "/api/v1/scores",
            serde_json::json!({ "character_id": 1, "score": 1000, "boss_reached": 0 }),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            get(app, "/api/v1/scores").await.1["scores"]
                .as_array()
                .unwrap()
                .len(),
            10
        );
    }

    #[tokio::test]
    async fn high_score_replaces_minimum_in_full_leaderboard() {
        let state = test_state().await;
        let app = router().with_state(state);

        fill_leaderboard(app.clone(), 1000).await;

        let (status, _) = post(
            app.clone(),
            "/api/v1/scores",
            serde_json::json!({ "character_id": 1, "score": 9999, "boss_reached": 1 }),
        )
        .await;

        assert_eq!(status, StatusCode::CREATED);

        let (_, body) = get(app, "/api/v1/scores").await;
        let scores = body["scores"].as_array().unwrap();
        assert_eq!(scores.len(), 10);
        assert_eq!(scores[0]["score"], 9999);
        // Minimum (1000) must have been evicted — lowest remaining is 1100
        assert!(scores.iter().all(|s| s["score"].as_i64().unwrap() >= 1100));
    }
}
