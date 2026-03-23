use axum::Router;
use axum::routing::get;
use utoipa::OpenApi;
use utoipa_scalar::{Scalar, Servable};

use crate::docs::ApiDoc;
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
        .merge(Scalar::with_url("/docs", ApiDoc::openapi()))
}
