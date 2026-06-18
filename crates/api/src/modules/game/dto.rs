use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct StartGameRequest {
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
            shield_active: snap.state.shield_active,
            boss_hp_percent: snap.boss_hp_percent,
        }
    }
}
