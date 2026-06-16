use std::time::{Duration, Instant};

use crate::engine::config::{
    STREAK_TIER_1_COUNT, STREAK_TIER_1_MULTIPLIER, STREAK_TIER_2_COUNT, STREAK_TIER_2_MULTIPLIER,
    STREAK_TIER_3_COUNT, STREAK_TIER_3_MULTIPLIER, STREAK_WINDOW_MS,
};

pub struct StreakState {
    count: u32,
    last_at: Option<Instant>,
}

impl StreakState {
    pub fn new() -> Self {
        Self {
            count: 0,
            last_at: None,
        }
    }

    /// Record a scoring event. Returns `true` if the multiplier tier changed.
    pub fn record(&mut self, now: Instant) -> bool {
        let prev_tier = self.tier();
        let in_window = self.last_at.is_some_and(|t| {
            now.duration_since(t) <= Duration::from_millis(STREAK_WINDOW_MS)
        });
        if in_window {
            self.count += 1;
        } else {
            self.count = 1;
        }
        self.last_at = Some(now);
        self.tier() != prev_tier
    }

    pub fn reset(&mut self) {
        self.count = 0;
        self.last_at = None;
    }

    fn tier(&self) -> u8 {
        if self.count >= STREAK_TIER_3_COUNT {
            3
        } else if self.count >= STREAK_TIER_2_COUNT {
            2
        } else if self.count >= STREAK_TIER_1_COUNT {
            1
        } else {
            0
        }
    }

    pub fn current(&self) -> f32 {
        match self.tier() {
            1 => STREAK_TIER_1_MULTIPLIER,
            2 => STREAK_TIER_2_MULTIPLIER,
            3 => STREAK_TIER_3_MULTIPLIER,
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
        assert!((s.current() - STREAK_TIER_2_MULTIPLIER).abs() < f32::EPSILON);
    }

    #[test]
    fn streak_resets_on_gap() {
        let mut s = StreakState::new();
        let now = Instant::now();
        for i in 0..10u64 {
            s.record(now + Duration::from_millis(i * 100));
        }
        // Gap beyond window
        s.record(now + Duration::from_millis(10 * 100 + STREAK_WINDOW_MS + 1));
        assert!((s.current() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn tier_change_detected() {
        let mut s = StreakState::new();
        let now = Instant::now();
        let mut tier_changed = false;
        for i in 0..STREAK_TIER_1_COUNT as u64 {
            tier_changed = s.record(now + Duration::from_millis(i * 100));
        }
        assert!(
            tier_changed,
            "should detect tier change at count={}",
            STREAK_TIER_1_COUNT
        );
    }
}
