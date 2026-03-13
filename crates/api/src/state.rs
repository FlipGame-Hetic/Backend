use std::sync::Arc;

use screen_hub::registry::ScreenRegistry;
use screen_hub::router::ScreenRouter;

use crate::modules::realtime::hub::BridgeHub;

/// Shared application state, injected into all Axum handlers via `State<AppState>`.
#[derive(Clone)]
pub struct AppState {
    pub hub: Arc<BridgeHub>,
    pub screen_registry: ScreenRegistry,
    pub screen_router: Arc<ScreenRouter>,
    pub jwt_secret: Arc<Vec<u8>>,
}

impl AppState {
    pub fn new(jwt_secret: Vec<u8>) -> Self {
        let screen_registry = ScreenRegistry::new();
        let screen_router = ScreenRouter::new(screen_registry.clone());

        Self {
            hub: Arc::new(BridgeHub::new()),
            screen_registry,
            screen_router: Arc::new(screen_router),
            jwt_secret: Arc::new(jwt_secret),
        }
    }
}