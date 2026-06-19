use std::time::Instant;

use crate::engine::components::health::HealthComponent;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PvePhase {
    Fighting,
    Cooldown,
    Victory,
    GameOver,
}

/// Tracks the timing of a boss defeat cooldown (death animation → next boss).
#[derive(Debug)]
pub struct CooldownState {
    /// Index of the next boss to spawn after cooldown.
    pub next_boss_index: u8,
    /// When the boss was defeated (start of death animation window).
    pub defeated_at: Instant,
    /// When BossCleared was emitted (`None` until death animation ends).
    pub cleared_at: Option<Instant>,
}

#[derive(Debug)]
pub struct PveState {
    pub current_boss_index: u8,
    pub endless_level: u32,
    pub boss_health: HealthComponent,
    pub phase: PvePhase,
    pub cooldown: Option<CooldownState>,
}

impl PveState {
    pub fn new(initial_hp: u32) -> Self {
        Self {
            current_boss_index: 0,
            endless_level: 0,
            boss_health: HealthComponent::new(initial_hp),
            phase: PvePhase::Fighting,
            cooldown: None,
        }
    }
}
