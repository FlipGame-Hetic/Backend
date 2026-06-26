//! Core data types shared across the combo subsystem.

use serde::{Deserialize, Serialize};

/// The two flipper buttons a player can press.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonPress {
    Left,
    Right,
}

/// Static definition of a single combo pattern, built from `GameConfig`.
#[derive(Debug, Clone)]
pub struct ComboDefinition {
    /// Unique identifier used in scoring events.
    pub id: u8,
    /// The exact sequence the player must press, in order.
    pub sequence: Vec<ButtonPress>,
    /// Max time in ms between the first and last press for the combo to count.
    pub max_duration_ms: u64,
    /// Flat bonus points awarded before multipliers are applied.
    pub bonus_pts: u32,
}

/// Data emitted when a combo fires, forwarded to the frontend for the animation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComboEffect {
    pub combo_id: u8,
    /// Raw bonus before the current score multiplier is applied.
    pub bonus_pts: u32,
    /// Human-readable sequence for the frontend ("L" / "R").
    pub sequence: Vec<String>,
}

/// Result of processing a single button press.
#[derive(Debug, Clone)]
pub enum ComboResult {
    /// A combo pattern was completed in time.
    Activated(ComboEffect),
    /// Player spammed the same button `pts` is applied as a score deduction.
    Penalty { pts: i64 },
    /// A hidden badge condition was met (reserved for future use).
    BadgeUnlocked { badge_id: String },
    /// Normal press — no combo matched, no penalty.
    None,
}
