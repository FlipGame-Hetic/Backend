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
