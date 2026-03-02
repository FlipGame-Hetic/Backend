use axum::routing::get;
use axum::Router;
use utoipa::OpenApi;
use utoipa_scalar::{Scalar, Servable};

use crate::docs::docs::ApiDoc;
use crate::modules::health;
use crate::modules::realtime::ws_handler;
use crate::state::AppState;

pub fn build() -> Router<AppState> {
    Router::new()
        .merge(health::routes::router())
        .route("/ws/bridge", get(ws_handler::ws_bridge))
        .merge(Scalar::with_url("/docs", ApiDoc::openapi()))
}