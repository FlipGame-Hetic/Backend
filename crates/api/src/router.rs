use axum::Router;
use axum::routing::get;
use lucy::docs_router;

use crate::modules::health;
use crate::modules::realtime::ws_handler;
use crate::modules::screen;
use crate::state::AppState;

pub fn build() -> Router<AppState> {
    Router::new()
        .merge(health::routes::router())
        .merge(screen::routes::router())
        .route("/ws/bridge", get(ws_handler::ws_bridge))
        .route("/ws/screen/{screen_id}", get(screen::ws_handler::ws_screen))
        // Lucy: interactive docs UI at /docs, spec at /docs/spec.json
        .merge(docs_router())
}
