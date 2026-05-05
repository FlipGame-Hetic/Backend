use crate::engine::config::{
    BUMPER_SCORE, BUMPER_TRIANGLE_SCORE, PORTAL_SCORE, TIMER_BONUS_MULTIPLIER, TIMER_BONUS_SCORE,
};

pub fn score_bumper(multiplier: f32) -> u64 {
    (BUMPER_SCORE as f32 * multiplier) as u64
}

pub fn score_bumper_triangle(multiplier: f32) -> u64 {
    (BUMPER_TRIANGLE_SCORE as f32 * multiplier) as u64
}

pub fn score_portal_bonus() -> u64 {
    PORTAL_SCORE as u64
}

pub fn apply_tilt_penalty(score: u64, penalty: i64) -> u64 {
    if penalty < 0 {
        score.saturating_sub(penalty.unsigned_abs())
    } else {
        score
    }
}

pub fn timer_bonus(score: u64, balls_lost: u32) -> u64 {
    if balls_lost == 0 {
        let with_bonus = score.saturating_add(TIMER_BONUS_SCORE as u64);
        (with_bonus as f32 * TIMER_BONUS_MULTIPLIER) as u64
    } else {
        score
    }
}

pub fn apply_multiplier(base: u64, multiplier: f32) -> u64 {
    (base as f32 * multiplier) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bumper_with_multiplier() {
        assert_eq!(score_bumper(1.5), 150);
    }

    #[test]
    fn test_bumper_triangle_with_multiplier() {
        assert_eq!(score_bumper_triangle(2.0), 400);
    }

    #[test]
    fn test_timer_bonus_no_lives_lost() {
        let result = timer_bonus(1_000, 0);
        // (1000 + 500) * 1.5 = 2250
        assert_eq!(result, 2_250);
    }

    #[test]
    fn test_timer_bonus_with_lives_lost() {
        assert_eq!(timer_bonus(1_000, 1), 1_000);
    }

    #[test]
    fn test_apply_tilt_penalty() {
        assert_eq!(apply_tilt_penalty(5_000, -2_000), 3_000);
    }

    #[test]
    fn test_apply_tilt_penalty_saturates() {
        assert_eq!(apply_tilt_penalty(100, -9_999), 0);
    }

    #[test]
    fn test_apply_multiplier() {
        assert_eq!(apply_multiplier(1_000, 2.0), 2_000);
    }
}
