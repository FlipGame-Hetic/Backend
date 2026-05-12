use axum::Router;
use axum::routing::get;
use lucyd::docs_router;

use crate::modules::game;
use crate::modules::health;
use crate::modules::realtime::ws_handler;
use crate::modules::scores;
use crate::modules::screen;
use crate::state::AppState;

pub fn build() -> Router<AppState> {
    Router::new()
        .merge(health::routes::router())
        .merge(screen::routes::router())
        .merge(game::routes::router())
        .merge(scores::routes::router())
        .route("/ws/bridge", get(ws_handler::ws_bridge))
        .route("/ws/screen/{screen_id}", get(screen::ws_handler::ws_screen))
        .merge(docs_router())
}
