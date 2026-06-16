use std::time::{Duration, Instant};

pub struct MultiplierState {
    active: Option<(f32, Instant, Duration)>,
}

impl MultiplierState {
    pub fn new() -> Self {
        Self { active: None }
    }

    pub fn apply(&mut self, factor: f32, duration_ms: u64, now: Instant) {
        self.active = Some((factor, now, Duration::from_millis(duration_ms)));
    }

    pub fn current(&self, now: Instant) -> f32 {
        match &self.active {
            Some((value, start, duration)) if now.duration_since(*start) <= *duration => *value,
            _ => 1.0,
        }
    }

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
