use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ScoreEntry {
    pub id: i64,
    pub player_id: String,
    pub character_id: i64,
    pub score: i64,
    pub boss_reached: i64,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct LeaderboardResponse {
    pub scores: Vec<ScoreEntry>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SaveScoreRequest {
    pub player_id: String,
    pub character_id: u8,
    pub score: u64,
    pub boss_reached: u8,
}
