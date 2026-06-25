use axum::Router;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use lucyd::lucy_http;

use game_logic::engine::config::{self, GameConfig};

use crate::errors::ApiError;
use crate::state::AppState;

use super::auth::AdminUser;
use super::service::AdminService;

pub fn router() -> Router<AppState> {
    use axum::routing::{get, patch};
    Router::new()
        .route("/api/v1/admin/config", get(get_config))
        .route("/api/v1/admin/config", patch(update_config))
}

#[lucy_http(
    method      = "GET",
    path        = "/api/v1/admin/config",
    tags        = "admin",
    response    = GameConfig,
    description = "Returns the current game engine configuration. Requires Authorization: Bearer <admin-jwt>.",
)]
pub async fn get_config(_admin: AdminUser) -> impl IntoResponse {
    (StatusCode::OK, axum::Json(config::get().clone()))
}

#[lucy_http(
    method      = "PATCH",
    path        = "/api/v1/admin/config",
    tags        = "admin",
    request     = GameConfig,
    response    = GameConfig,
    description = "Update the game engine configuration. Changes are persisted to the database and applied immediately — no restart required. Requires Authorization: Bearer <admin-jwt>.",
)]
pub async fn update_config(
    _admin: AdminUser,
    State(state): State<AppState>,
    axum::Json(body): axum::Json<GameConfig>,
) -> Result<impl IntoResponse, ApiError> {
    AdminService::save_config(&state.db_pool, &body).await?;
    game_logic::engine::config::set(body.clone());
    Ok((StatusCode::OK, axum::Json(body)))
}
