use std::collections::HashMap;
use std::sync::Arc;

use game_logic::GameEngine;
use screen_hub::registry::ScreenRegistry;
use screen_hub::router::ScreenRouter;
use tokio::sync::Mutex;

use crate::modules::menu::state_machine::MenuStateMachine;
use crate::modules::realtime::hub::BridgeHub;

/// Identifies a single rail scoring session, unique per ball.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RailSessionKey {
    pub ball_id: Option<String>,
}

/// Identifies the current player session between StartGame and GameOver.
#[derive(Debug, Clone)]
pub struct GameSession {
    pub player_id: String,
    pub character_id: u8,
    pub boss_reached: u8,
}

/// Shared application state, injected into all Axum handlers via `State<AppState>`.
#[derive(Clone)]
pub struct AppState {
    pub hub: Arc<BridgeHub>,
    pub screen_registry: ScreenRegistry,
    pub screen_router: Arc<ScreenRouter>,
    pub jwt_secret: Arc<Vec<u8>>,
    /// Lock order: always acquire game_engine FIRST, active_session SECOND.
    pub game_engine: Arc<Mutex<Option<GameEngine>>>,
    pub active_session: Arc<Mutex<Option<GameSession>>>,
    pub active_device_id: Arc<tokio::sync::RwLock<Option<String>>>,
    pub db_pool: sqlx::SqlitePool,
    /// Active rail/ramp ticker sessions. Dropping a sender cancels the associated task.
    pub active_rail_sessions: Arc<Mutex<HashMap<RailSessionKey, tokio::sync::oneshot::Sender<()>>>>,
    pub menu_state: Arc<Mutex<MenuStateMachine>>,
}

impl AppState {
    pub fn new(jwt_secret: Vec<u8>, db_pool: sqlx::SqlitePool) -> Self {
        let screen_registry = ScreenRegistry::new();
        let screen_router = ScreenRouter::new(screen_registry.clone());

        Self {
            hub: Arc::new(BridgeHub::new()),
            screen_registry,
            screen_router: Arc::new(screen_router),
            jwt_secret: Arc::new(jwt_secret),
            game_engine: Arc::new(Mutex::new(None)),
            active_session: Arc::new(Mutex::new(None)),
            active_device_id: Arc::new(tokio::sync::RwLock::new(None)),
            db_pool,
            active_rail_sessions: Arc::new(Mutex::new(HashMap::new())),
            menu_state: Arc::new(Mutex::new(MenuStateMachine::new())),
        }
    }
}
