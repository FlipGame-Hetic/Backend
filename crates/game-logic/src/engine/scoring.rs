use crate::engine::config;

pub fn score_bumper(multiplier: f32) -> u64 {
    (config::get().bumper_score as f32 * multiplier) as u64
}

pub fn score_bumper_triangle(multiplier: f32) -> u64 {
    (config::get().bumper_triangle_score as f32 * multiplier) as u64
}

pub fn score_portal_bonus() -> u64 {
    config::get().portal_score as u64
}

pub fn apply_tilt_penalty(score: u64, penalty: i64) -> u64 {
    if penalty < 0 {
        score.saturating_sub(penalty.unsigned_abs())
    } else {
        score
    }
}

pub fn timer_bonus(score: u64, balls_lost: u32) -> u64 {
    let cfg = config::get();
    if balls_lost == 0 {
        let with_bonus = score.saturating_add(cfg.timer_bonus_score as u64);
        (with_bonus as f32 * cfg.timer_bonus_multiplier) as u64
    } else {
        score
    }
}

pub fn apply_multiplier(base: u64, multiplier: f32) -> u64 {
    (base as f32 * multiplier) as u64
}

/// Returns the nth Fibonacci number (1-indexed: F(0)=1, F(1)=1, F(2)=2, ...).
pub fn fibonacci(n: u32) -> u64 {
    if n <= 1 {
        return 1;
    }
    let mut a: u64 = 1;
    let mut b: u64 = 1;
    for _ in 2..=n {
        let next = a.saturating_add(b);
        a = b;
        b = next;
    }
    b
}

pub fn rail_tick_score(fib_step: u32, multiplier: f32) -> u64 {
    let cfg = config::get();
    let fib = fibonacci(fib_step.min(cfg.rail_max_fib_step)) as f32;
    (cfg.rail_base_score as f32 * fib * multiplier) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bumper_with_multiplier() {
        let bumper_score = config::get().bumper_score;
        assert_eq!(score_bumper(1.5), (bumper_score as f32 * 1.5) as u64);
    }

    #[test]
    fn test_bumper_triangle_with_multiplier() {
        let bumper_triangle_score = config::get().bumper_triangle_score;
        assert_eq!(
            score_bumper_triangle(2.0),
            (bumper_triangle_score as f32 * 2.0) as u64
        );
    }

    #[test]
    fn test_timer_bonus_no_lives_lost() {
        let cfg = config::get();
        let expected =
            ((1_000 + cfg.timer_bonus_score as u64) as f32 * cfg.timer_bonus_multiplier) as u64;
        assert_eq!(timer_bonus(1_000, 0), expected);
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

    #[test]
    fn test_fibonacci_base_cases() {
        assert_eq!(fibonacci(0), 1);
        assert_eq!(fibonacci(1), 1);
    }

    #[test]
    fn test_fibonacci_sequence() {
        // 1, 1, 2, 3, 5, 8, 13, 21, 34, 55, 89
        let expected = [1u64, 1, 2, 3, 5, 8, 13, 21, 34, 55, 89];
        for (i, &v) in expected.iter().enumerate() {
            assert_eq!(fibonacci(i as u32), v, "fibonacci({i}) should be {v}");
        }
    }

    #[test]
    fn test_rail_tick_score_step_0_no_multiplier() {
        let rail_base_score = config::get().rail_base_score;
        assert_eq!(rail_tick_score(0, 1.0), rail_base_score as u64);
    }

    #[test]
    fn test_rail_tick_score_step_4_with_multiplier() {
        let rail_base_score = config::get().rail_base_score;
        assert_eq!(
            rail_tick_score(4, 2.0),
            (rail_base_score as f32 * fibonacci(4) as f32 * 2.0) as u64
        );
    }

    #[test]
    fn test_rail_tick_score_capped_at_max_fib_step() {
        let rail_max_fib_step = config::get().rail_max_fib_step;
        assert_eq!(
            rail_tick_score(100, 1.0),
            rail_tick_score(rail_max_fib_step, 1.0)
        );
    }
}
