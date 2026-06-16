pub mod combo;
pub mod engine;
pub mod player;

pub use combo::{ComboDetector, ComboEffect, ComboResult};
pub use engine::core::GameEngine;
pub use engine::events::{ButtonSide, GameEvent, GameOverReason};
pub use engine::states::{GamePhase, GameState};
pub use player::personnages::character::{Character, select_character};

/// Aggregated view of the engine for the HTTP layer.
///
/// `GameState` alone does not expose the authoritative multiplier (held in
/// `MultiplierState`) nor the boss HP (held in `PveEngine`). This snapshot
/// bundles all three so the API response is always consistent.
#[derive(Debug, Clone)]
pub struct GameSnapshot {
    pub state: GameState,
    pub current_multiplier: f32,
    pub boss_hp_percent: Option<f32>,
}
