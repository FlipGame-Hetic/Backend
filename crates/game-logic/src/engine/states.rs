use std::time::Instant;

use crate::engine::config::{TILT_PENALTY_1, TILT_PENALTY_2};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GamePhase {
    Idle,
    InGame,
    GameOver,
}

#[derive(Debug, Clone)]
pub enum TiltEffect {
    Penalty(i64),
    CheatingDetected,
}

#[derive(Debug, Clone, Default)]
pub struct TiltState {
    pub count: u8,
}

impl TiltState {
    pub fn on_tilt(&mut self) -> TiltEffect {
        self.count += 1;
        match self.count {
            1 => TiltEffect::Penalty(TILT_PENALTY_1),
            2 => TiltEffect::Penalty(TILT_PENALTY_2),
            _ => TiltEffect::CheatingDetected,
        }
    }

    pub fn reset(&mut self) {
        self.count = 0;
    }
}

#[derive(Debug, Clone)]
pub struct GameState {
    pub phase: GamePhase,
    pub score: u64,
    pub lives: u8,
    pub tilt_state: TiltState,
    pub balls_lost_since_start: u32,
    pub session_start: Option<Instant>,
    pub cheating_detected: bool,
    pub extra_balls: u8,
    pub shield_active: bool,
    pub shield_expires_at: Option<Instant>,
    pub damage_multiplier: f32,
    pub multiball_active: bool,

    // Ultimate charge
    pub ultimate_charge: u32,
    /// Sub-100-point remainder carried over between scoring events.
    pub point_buffer: u32,
    /// Fractional accumulator for time-based charge (Oracle).
    pub time_charge_buffer: f32,

    // Ulti state machine (lazy eval — no timers)
    pub ulti_ends_at: Option<Instant>,
    pub ulti_duration_ms: u64,
    pub ulti_cancellable: bool,
    pub ulti_active_id: Option<String>,
    /// Forces `effective_multiplier` to exactly this value while ulti is active (Viper).
    pub ulti_multiplier_override: Option<f32>,

    // Ghost cycle (reset on StartGame)
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
            ulti_multiplier_override: None,
            ghost_cycle_index: 0,
        }
    }

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

    pub fn is_ulti_active(&self, now: Instant) -> bool {
        self.ulti_ends_at.map(|t| now < t).unwrap_or(false)
    }

    /// Remaining charge at `now` during a sustained ulti (linear drain).
    pub fn residual_charge(&self, now: Instant) -> u32 {
        let Some(ends_at) = self.ulti_ends_at else {
            return 0;
        };
        if now >= ends_at || self.ulti_duration_ms == 0 {
            return 0;
        }
        // We can't store charge_max here, so callers pass it.
        // This method only computes the fraction; the caller multiplies by charge_max.
        // Keep it simple: store nothing extra — callers use residual_charge_with_max.
        0
    }

    pub fn residual_charge_with_max(&self, now: Instant, charge_max: u32) -> u32 {
        let Some(ends_at) = self.ulti_ends_at else {
            return 0;
        };
        if now >= ends_at || self.ulti_duration_ms == 0 {
            return 0;
        }
        let remaining_ms = ends_at.duration_since(now).as_millis() as u64;
        ((charge_max as u64 * remaining_ms) / self.ulti_duration_ms) as u32
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
