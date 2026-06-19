use std::time::Instant;

use crate::engine::components::health::HealthComponent;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PvePhase {
    /// Waiting for BOSS_SCORE_THRESHOLD points before spawning the next boss.
    WaitingForScore,
    Fighting,
    /// Death animation window after a boss is defeated.
    Cooldown,
    Victory,
    GameOver,
}

/// Tracks the death animation window after a boss defeat.
#[derive(Debug)]
pub struct CooldownState {
    /// Index of the next boss to spawn after cooldown.
    pub next_boss_index: u8,
    /// When the boss was defeated (start of death animation window).
    pub defeated_at: Instant,
}

#[derive(Debug)]
pub struct PveState {
    pub current_boss_index: u8,
    /// Boss index to spawn once the score threshold is reached.
    pub next_boss_index: u8,
    pub endless_level: u32,
    pub boss_health: HealthComponent,
    pub phase: PvePhase,
    pub cooldown: Option<CooldownState>,
    /// Points accumulated since entering WaitingForScore.
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
