use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Body for `POST /api/v1/game/start`
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct StartGameRequest {
    /// Character slug one of: `enforcer`, `viper`, `ghost`, `oracle`
    pub character: String,
}

/// Snapshot of the game engine state returned by start / state / end endpoints
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct GameStateResponse {
    /// One of: `"idle"`, `"in_game"`, `"game_over"`
    pub phase: String,
    pub score: u64,
    /// Remaining lives (starts at 3, game over at 0)
    pub lives: u8,
    pub active_multiplier: f32,
    pub ultimate_charge: u32,
    /// Maximum charge capacity for the active character's ultimate filled by the route handler
    pub ultimate_max: u32,
    pub shield_active: bool,
    /// `None` when no boss fight is active
    pub boss_hp_percent: Option<f32>,
    /// Active character slug; absent when no game is in progress
    #[serde(skip_serializing_if = "Option::is_none")]
    pub character: Option<String>,
}

impl From<game_logic::GameSnapshot> for GameStateResponse {
    fn from(snap: game_logic::GameSnapshot) -> Self {
        let phase = match snap.state.phase {
            game_logic::GamePhase::Idle => "idle",
            game_logic::GamePhase::InGame => "in_game",
            game_logic::GamePhase::GameOver => "game_over",
        };
        Self {
            phase: phase.to_owned(),
            score: snap.state.score,
            lives: snap.state.lives,
            active_multiplier: snap.current_multiplier,
            ultimate_charge: snap.state.ultimate_charge,
            ultimate_max: 0, // filled by the service layer which knows the character
            shield_active: snap.state.shield_active,
            boss_hp_percent: snap.boss_hp_percent,
            character: None, // populated by the route handler from active_session
        }
    }
}
