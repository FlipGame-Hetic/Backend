use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct StartGameRequest {
    pub player_id: String,
    pub character_id: u8,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct GameStateResponse {
    pub phase: String,
    pub score: u64,
    pub lives: u8,
    pub active_multiplier: f32,
    pub ultimate_charge: u32,
    pub shield_active: bool,
    pub boss_hp_percent: Option<f32>,
}

impl From<game_logic::GameState> for GameStateResponse {
    fn from(s: game_logic::GameState) -> Self {
        let phase = match s.phase {
            game_logic::GamePhase::Idle => "idle",
            game_logic::GamePhase::InGame => "in_game",
            game_logic::GamePhase::GameOver => "game_over",
        };
        Self {
            phase: phase.to_owned(),
            score: s.score,
            lives: s.lives,
            active_multiplier: s.active_multiplier,
            ultimate_charge: s.ultimate_charge,
            shield_active: s.shield_active,
            boss_hp_percent: None,
        }
    }
}
