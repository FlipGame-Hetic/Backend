use axum::Router;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use lucyd::lucy_http;

use game_logic::engine::config::{self, GameConfig, GameConfigPatch};

use crate::errors::ApiError;
use crate::state::AppState;

use super::auth::AdminUser;
use super::service::AdminService;

pub fn router() -> Router<AppState> {
    use axum::routing::{get, patch, put};
    Router::new()
        .route("/api/v1/admin/config", get(get_config))
        .route("/api/v1/admin/config", put(update_config))
        .route("/api/v1/admin/config", patch(patch_config))
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
    method      = "PUT",
    path        = "/api/v1/admin/config",
    tags        = "admin",
    request     = GameConfig,
    response    = GameConfig,
    description = "Replace the entire game engine configuration. All fields are required. Changes are persisted and applied immediately. Requires Authorization: Bearer <admin-jwt>.",
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

#[lucy_http(
    method      = "PATCH",
    path        = "/api/v1/admin/config",
    tags        = "admin",
    request     = GameConfigPatch,
    response    = GameConfig,
    description = "Partially update the game engine configuration. Only provided fields are updated; omitted fields keep their current value. Changes are persisted and applied immediately. Requires Authorization: Bearer <admin-jwt>.",
)]
pub async fn patch_config(
    _admin: AdminUser,
    State(state): State<AppState>,
    axum::Json(patch): axum::Json<GameConfigPatch>,
) -> Result<impl IntoResponse, ApiError> {
    // Apply the patch atomically in memory first, then read back the full config to persist.
    // This guarantees the DB always stores the complete merged state, not just the delta
    game_logic::engine::config::apply_patch(patch);
    let updated = config::get().clone();
    AdminService::save_config(&state.db_pool, &updated).await?;
    Ok((StatusCode::OK, axum::Json(updated)))
}
