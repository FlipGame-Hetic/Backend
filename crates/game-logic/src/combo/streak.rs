//! Consecutive-hit streak tracker that boosts the score multiplier.
//! The streak resets when the gap between two scoring events exceeds `streak_window_ms`.

use std::time::{Duration, Instant};

use crate::engine::config;

/// Tracks how many scoring events happened consecutively within the time window.
pub struct StreakState {
    count: u32,
    /// Timestamp of the last recorded event, used to detect gaps.
    last_at: Option<Instant>,
}

impl StreakState {
    pub fn new() -> Self {
        Self {
            count: 0,
            last_at: None,
        }
    }

    /// Record a scoring event and update the streak counter.
    ///
    /// Returns `(tier_changed, streak_armed)`:
    /// - `tier_changed` the multiplier tier changed, emit a `MultiplierUpdate`
    /// - `streak_armed` tier > 0, a multiplier is now active
    pub fn record(&mut self, now: Instant) -> (bool, bool) {
        let prev_tier = self.tier();
        let window_ms = config::get().streak_window_ms;

        let in_window = self
            .last_at
            .is_some_and(|t| now.duration_since(t) <= Duration::from_millis(window_ms));

        if in_window {
            self.count += 1;
        } else {
            // Gap too large — restart the streak at 1 (this event is the new start).
            self.count = 1;
        }
        self.last_at = Some(now);

        let new_tier = self.tier();
        (new_tier != prev_tier, new_tier > 0)
    }

    /// Reset to zero called on ball loss so streaks don't carry across balls.
    pub fn reset(&mut self) {
        self.count = 0;
        self.last_at = None;
    }

    /// Map the current hit count to a tier (0–3) using config thresholds.
    fn tier(&self) -> u8 {
        let cfg = config::get();
        if self.count >= cfg.streak_tier_3_count {
            3
        } else if self.count >= cfg.streak_tier_2_count {
            2
        } else if self.count >= cfg.streak_tier_1_count {
            1
        } else {
            0
        }
    }

    /// Return the multiplier factor for the current tier (1.0 when no streak).
    pub fn current(&self) -> f32 {
        let cfg = config::get();
        match self.tier() {
            1 => cfg.streak_tier_1_multiplier,
            2 => cfg.streak_tier_2_multiplier,
            3 => cfg.streak_tier_3_multiplier,
            _ => 1.0,
        }
    }
}

impl Default for StreakState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_streak_returns_1x() {
        let s = StreakState::new();
        assert!((s.current() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn streak_builds_within_window() {
        let mut s = StreakState::new();
        let now = Instant::now();
        for i in 0..6u64 {
            s.record(now + Duration::from_millis(i * 100));
        }
        let tier_2_multiplier = config::get().streak_tier_2_multiplier;
        assert!((s.current() - tier_2_multiplier).abs() < f32::EPSILON);
    }

    #[test]
    fn streak_resets_on_gap() {
        let mut s = StreakState::new();
        let now = Instant::now();
        for i in 0..10u64 {
            s.record(now + Duration::from_millis(i * 100));
        }
        let window_ms = config::get().streak_window_ms;
        s.record(now + Duration::from_millis(10 * 100 + window_ms + 1));
        assert!((s.current() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn tier_change_detected() {
        let mut s = StreakState::new();
        let now = Instant::now();
        let tier_1_count = config::get().streak_tier_1_count;
        let mut tier_changed = false;
        for i in 0..tier_1_count as u64 {
            let (tc, _) = s.record(now + Duration::from_millis(i * 100));
            tier_changed = tc;
        }
        assert!(
            tier_changed,
            "should detect tier change at count={}",
            tier_1_count
        );
    }
}
