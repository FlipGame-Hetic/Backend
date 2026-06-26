use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A single leaderboard row as returned by the database
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ScoreEntry {
    pub id: i64,
    /// DB integer ID matching `game_logic::slug_to_db_id` (1=enforcer, 2=viper, 3=ghost, 4=oracle)
    pub character_id: i64,
    pub score: i64,
    /// Number of bosses defeated during the run
    pub boss_reached: i64,
    /// ISO-8601 timestamp inserted by SQLite `datetime('now')`, `None` on very old rows
    pub created_at: Option<String>,
}

/// Top-N leaderboard as returned by `GET /api/v1/scores`
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct LeaderboardResponse {
    pub scores: Vec<ScoreEntry>,
}

/// Body for the debug `POST /api/v1/scores` endpoint
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SaveScoreRequest {
    /// See `ScoreEntry::character_id` for the mapping
    pub character_id: u8,
    pub score: u64,
    pub boss_reached: u8,
}
