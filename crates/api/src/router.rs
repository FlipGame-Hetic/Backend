use axum::Router;
use axum::routing::get;
use lucyd::docs_router;

use crate::modules::admin;
use crate::modules::game;
use crate::modules::health;
use crate::modules::realtime::ws_handler;
use crate::modules::scores;
use crate::modules::screen;
use crate::state::AppState;

/// Merge all sub-routers and register the two WebSocket upgrade endpoints
///
/// Route layout:
/// - `/health`                  — liveness probe
/// - `/api/v1/game/*`           — game lifecycle (start / state / end / characters)
/// - `/api/v1/scores`           — leaderboard
/// - `/api/v1/admin/config`     — live game config (admin-JWT protected)
/// - `/api/v1/screens/*`        — screen registry debug endpoints
/// - `/ws/bridge`               — MQTT bridge WebSocket
/// - `/ws/screen/{screen_id}`   — per-screen WebSocket (JWT authenticated)
/// - `/docs`                    — Lucyd auto-generated API docs
pub fn build() -> Router<AppState> {
    Router::new()
        .merge(health::routes::router())
        .merge(screen::routes::router())
        .merge(game::routes::router())
        .merge(scores::routes::router())
        .merge(admin::routes::router())
        .route("/ws/bridge", get(ws_handler::ws_bridge))
        .route("/ws/screen/{screen_id}", get(screen::ws_handler::ws_screen))
        .merge(docs_router())
}
