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
    pub active_multiplier: f32,
    pub multiplier_expires_at: Option<Instant>,
    pub tilt_state: TiltState,
    pub balls_lost_since_start: u32,
    pub session_start: Option<Instant>,
    pub cheating_detected: bool,
    pub extra_balls: u8,
    pub ultimate_charge: u32,
    pub shield_active: bool,
    pub shield_expires_at: Option<Instant>,
    pub damage_multiplier: f32,
}

impl GameState {
    pub fn new(lives: u8) -> Self {
        Self {
            phase: GamePhase::Idle,
            score: 0,
            lives,
            active_multiplier: 1.0,
            multiplier_expires_at: None,
            tilt_state: TiltState::default(),
            balls_lost_since_start: 0,
            session_start: None,
            cheating_detected: false,
            extra_balls: 0,
            ultimate_charge: 0,
            shield_active: false,
            shield_expires_at: None,
            damage_multiplier: 1.0,
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
