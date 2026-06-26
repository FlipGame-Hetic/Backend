//! PVE phase machine and associated state structs.

use std::time::Instant;

use crate::engine::components::health::HealthComponent;

/// Current phase of the PVE encounter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PvePhase {
    /// Accumulating score toward the next boss spawn threshold.
    WaitingForScore,
    /// A boss is alive — score deltas deal damage.
    Fighting,
    /// Death animation window after a boss is defeated; score deltas are ignored.
    Cooldown,
    Victory,
    GameOver,
}

/// Tracks the death animation window after a boss defeat.
#[derive(Debug)]
pub struct CooldownState {
    /// Index of the next boss to spawn once the animation finishes.
    pub next_boss_index: u8,
    /// When the boss was defeated — used to measure elapsed animation time.
    pub defeated_at: Instant,
}

/// Full PVE state owned by `PveEngine`.
#[derive(Debug)]
pub struct PveState {
    pub current_boss_index: u8,
    /// Boss index to spawn once the score threshold is reached after a kill.
    pub next_boss_index: u8,
    /// How many endless cycles have been completed after all story bosses are defeated.
    pub endless_level: u32,
    pub boss_health: HealthComponent,
    pub phase: PvePhase,
    pub cooldown: Option<CooldownState>,
    /// Points accumulated since entering `WaitingForScore` (resets on boss spawn).
    pub score_accumulated: u64,
}

impl PveState {
    pub fn new(initial_hp: u32) -> Self {
        Self {
            current_boss_index: 0,
            next_boss_index: 0,
            endless_level: 0,
            boss_health: HealthComponent::new(initial_hp),
            phase: PvePhase::WaitingForScore,
            cooldown: None,
            score_accumulated: 0,
        }
    }
}
