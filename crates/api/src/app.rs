use axum::Router;
use sqlx::SqlitePool;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::config::ApiConfig;
use crate::router;
use crate::state::AppState;

pub fn build(config: &ApiConfig, db_pool: SqlitePool) -> Router {
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
