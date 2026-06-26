//! Game-wide state types: phase machine, tilt tracker, and the full `GameState`.

use std::time::Instant;

use crate::engine::config;

/// Lifecycle phase of a game session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GamePhase {
    Idle,
    InGame,
    GameOver,
}

/// What happens when the pinball machine tilts.
#[derive(Debug, Clone)]
pub enum TiltEffect {
    /// Deduct `pts` from the score (negative value).
    Penalty(i64),
    /// Third tilt in a session — score is locked for the rest of the game.
    CheatingDetected,
}

/// Counts how many times the machine tilted this session.
/// First tilt → small penalty, second → large penalty, third+ → cheating lock.
#[derive(Debug, Clone, Default)]
pub struct TiltState {
    pub count: u8,
}

impl TiltState {
    pub fn on_tilt(&mut self) -> TiltEffect {
        self.count += 1;
        let cfg = config::get();
        match self.count {
            1 => TiltEffect::Penalty(cfg.tilt_penalty_1),
            2 => TiltEffect::Penalty(cfg.tilt_penalty_2),
            _ => TiltEffect::CheatingDetected,
        }
    }

    pub fn reset(&mut self) {
        self.count = 0;
    }
}

/// Full mutable state of a running game session.
/// Owned by `GameEngine` and cloned into `GameSnapshot` for API responses.
#[derive(Debug, Clone)]
pub struct GameState {
    pub phase: GamePhase,
    pub score: u64,
    pub lives: u8,
    pub tilt_state: TiltState,
    pub balls_lost_since_start: u32,
    pub session_start: Option<Instant>,
    /// When true, `add_score` becomes a no-op triggered by the third tilt.
    pub cheating_detected: bool,
    pub extra_balls: u8,
    pub shield_active: bool,
    pub shield_expires_at: Option<Instant>,
    pub damage_multiplier: f32,
    pub multiball_active: bool,

    // Ultimate charge
    pub ultimate_charge: u32,
    /// Sub-threshold remainder carried between scoring events to avoid rounding loss.
    pub point_buffer: u32,
    /// Fractional accumulator for time-based charge (Oracle character only).
    pub time_charge_buffer: f32,

    // Ulti state machine expiry checked lazily on each `process` call, no background timer.
    pub ulti_ends_at: Option<Instant>,
    pub ulti_duration_ms: u64,
    pub ulti_cancellable: bool,
    pub ulti_active_id: Option<String>,
    /// Charge committed at activation — used to compute residual on cancel/display.
    pub ulti_start_charge: u32,
    /// Overrides `effective_multiplier` to a fixed value while ulti is active (Viper rampage).
    pub ulti_multiplier_override: Option<f32>,

    /// Which ulti Ghost will fire next (cycles through 3 options, resets on StartGame).
    pub ghost_cycle_index: u8,
}

impl GameState {
    pub fn new(lives: u8) -> Self {
        Self {
            phase: GamePhase::Idle,
            score: 0,
            lives,
            tilt_state: TiltState::default(),
            balls_lost_since_start: 0,
            session_start: None,
            cheating_detected: false,
            extra_balls: 0,
            shield_active: false,
            shield_expires_at: None,
            damage_multiplier: 1.0,
            multiball_active: false,
            ultimate_charge: 0,
            point_buffer: 0,
            time_charge_buffer: 0.0,
            ulti_ends_at: None,
            ulti_duration_ms: 0,
            ulti_cancellable: false,
            ulti_active_id: None,
            ulti_start_charge: 0,
            ulti_multiplier_override: None,
            ghost_cycle_index: 0,
        }
    }

    /// Add points to the score. Silently ignored when cheating is detected.
    pub fn add_score(&mut self, pts: u64) {
        if !self.cheating_detected {
            self.score = self.score.saturating_add(pts);
        }
    }

    pub fn apply_penalty(&mut self, penalty: i64) {
        if penalty < 0 {
            self.score = self.score.saturating_sub(penalty.unsigned_abs());
        }
    }

    /// Return `true` if a sustained ulti is currently running (not yet expired).
    pub fn is_ulti_active(&self, now: Instant) -> bool {
        self.ulti_ends_at.map(|t| now < t).unwrap_or(false)
    }

    /// Compute remaining charge during a sustained ulti (linear drain from `ulti_start_charge`).
    /// Used both to display the charge bar and to restore partial charge on cancel.
    pub fn residual_charge(&self, now: Instant) -> u32 {
        let Some(ends_at) = self.ulti_ends_at else {
            return 0;
        };
        if now >= ends_at || self.ulti_duration_ms == 0 {
            return 0;
        }
        let remaining_ms = ends_at.duration_since(now).as_millis() as u64;
        ((self.ulti_start_charge as u64 * remaining_ms) / self.ulti_duration_ms) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tilt_first() {
        let mut ts = TiltState::default();
        assert!(matches!(ts.on_tilt(), TiltEffect::Penalty(-2_000)));
    }

    #[test]
    fn test_tilt_second() {
        let mut ts = TiltState::default();
        ts.on_tilt();
        assert!(matches!(ts.on_tilt(), TiltEffect::Penalty(-6_000)));
    }

    #[test]
    fn test_tilt_third() {
        let mut ts = TiltState::default();
        ts.on_tilt();
        ts.on_tilt();
        assert!(matches!(ts.on_tilt(), TiltEffect::CheatingDetected));
    }
}
