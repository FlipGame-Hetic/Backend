use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonPress {
    Left,
    Right,
}

#[derive(Debug, Clone)]
pub struct ComboDefinition {
    pub id: u8,
    pub sequence: Vec<ButtonPress>,
    pub max_duration_ms: u64,
    pub bonus_pts: u32,
    pub multiplier: f32,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComboEffect {
    pub combo_id: u8,
    pub bonus_pts: u32,
    pub multiplier: f32,
    pub duration_ms: u64,
}

#[derive(Debug, Clone)]
pub enum ComboResult {
    Activated(ComboEffect),
    Penalty { pts: i64 },
    BadgeUnlocked { badge_id: String },
    None,
}
