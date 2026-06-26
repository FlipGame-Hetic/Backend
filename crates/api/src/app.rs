use axum::Router;
use sqlx::SqlitePool;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::config::ApiConfig;
use crate::router;
use crate::state::AppState;

/// Assemble the Axum router with CORS, tracing middleware, and shared state
///
/// Called once at startup; the returned `Router` is handed directly to `axum::serve`
pub fn build(config: &ApiConfig, db_pool: SqlitePool) -> Router {
    // A single "*" in ALLOWED_ORIGINS means open CORS (dev/CI use).
    // Any other value is treated as an explicit allowlist — invalid entries are silently skipped
    let cors = if config.allowed_origins.iter().any(|o| o == "*") {
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any)
    } else {
        CorsLayer::new()
            .allow_origin(
                config
                    .allowed_origins
                    .iter()
                    .filter_map(|o| o.parse().ok())
                    .collect::<Vec<_>>(),
            )
            .allow_methods(Any)
            .allow_headers(Any)
    };

    let state = AppState::new(config.jwt_secret.as_bytes().to_vec(), db_pool);

    router::build()
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state)
}
