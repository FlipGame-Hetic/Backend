use std::sync::Arc;

use crate::modules::realtime::hub::BridgeHub;

/// Shared application state, injected into all Axum handlers via `State<AppState>`.
#[derive(Clone)]
pub struct AppState {
    pub hub: Arc<BridgeHub>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            hub: Arc::new(BridgeHub::new()),
        }
    }
}