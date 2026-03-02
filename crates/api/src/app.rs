use axum::Router;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::config::ApiConfig;
use crate::router;
use crate::state::AppState;

pub fn build(config: &ApiConfig) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(
            config
                .allowed_origins
                .iter()
                .filter_map(|o| o.parse().ok())
                .collect::<Vec<_>>(),
        )
        .allow_methods(Any)
        .allow_headers(Any);

    let state = AppState::new();

    router::build()
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state)
}
