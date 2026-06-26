//! Time-limited score multiplier (e.g. activated by a character ultimate).

use std::time::{Duration, Instant};

/// Tracks a single active score multiplier with an expiry time.
/// Only one multiplier can be active calling `apply` replaces any existing one.
pub struct MultiplierState {
    /// `Some((factor, start, duration))` while active, `None` otherwise.
    active: Option<(f32, Instant, Duration)>,
}

impl MultiplierState {
    pub fn new() -> Self {
        Self { active: None }
    }

    /// Start a boost of `factor` lasting `duration_ms` milliseconds from `now`.
    pub fn apply(&mut self, factor: f32, duration_ms: u64, now: Instant) {
        self.active = Some((factor, now, Duration::from_millis(duration_ms)));
    }

    /// Return the current multiplier factor, or `1.0` if expired / never set.
    pub fn current(&self, now: Instant) -> f32 {
        match &self.active {
            Some((value, start, duration)) if now.duration_since(*start) <= *duration => *value,
            _ => 1.0,
        }
    }

    /// Return `true` if no boost is active or the active one has expired.
    pub fn is_expired(&self, now: Instant) -> bool {
        match &self.active {
            Some((_, start, duration)) => now.duration_since(*start) > *duration,
            None => true,
        }
    }
}

impl Default for MultiplierState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn test_multiplier_active() {
        let mut ms = MultiplierState::new();
        let now = Instant::now();
        ms.apply(2.0, 5_000, now);
        assert!((ms.current(now) - 2.0).abs() < f32::EPSILON);
        assert!(!ms.is_expired(now));
    }

    #[test]
    fn test_multiplier_expires() {
        let mut ms = MultiplierState::new();
        let past = Instant::now() - Duration::from_millis(6_000);
        ms.apply(2.0, 1_000, past);
        assert!((ms.current(Instant::now()) - 1.0).abs() < f32::EPSILON);
        assert!(ms.is_expired(Instant::now()));
    }
}
