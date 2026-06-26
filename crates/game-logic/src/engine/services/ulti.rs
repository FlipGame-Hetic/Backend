/// Minimum charge required to activate a time_slow ulti at `ratio` of `charge_max`.
/// Always at least 1 to prevent a zero threshold.
pub fn activation_min_charge_for(charge_max: u32, ratio: f32) -> u32 {
    ((charge_max as f32 * ratio).ceil() as u32).max(1)
}

/// Scale a full ulti duration down to the fraction of charge actually committed.
/// Returns at least 1 ms. At `charge_max` the result equals `full_duration_ms`.
pub fn scale_duration(full_duration_ms: u64, activation_charge: u32, charge_max: u32) -> u64 {
    if charge_max == 0 {
        return full_duration_ms;
    }
    (full_duration_ms * activation_charge as u64 / charge_max as u64).max(1)
}

/// Linear drain: how much charge remains given `remaining_ms` left out of `duration_ms`.
/// `start_charge` is the charge committed at activation.
pub fn residual_charge(start_charge: u32, remaining_ms: u64, duration_ms: u64) -> u32 {
    if duration_ms == 0 {
        return 0;
    }
    let clamped = remaining_ms.min(duration_ms);
    ((start_charge as u64 * clamped) / duration_ms) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    // activation_min_charge_for

    #[test]
    fn min_charge_5_percent_of_80() {
        // ceil(80 * 0.05) = ceil(4.0) = 4
        assert_eq!(activation_min_charge_for(80, 0.05), 4);
    }

    #[test]
    fn min_charge_rounds_up() {
        // ceil(80 * 0.06) = ceil(4.8) = 5
        assert_eq!(activation_min_charge_for(80, 0.06), 5);
    }

    #[test]
    fn min_charge_zero_ratio_returns_one() {
        // ceil(0) = 0, clamped to 1
        assert_eq!(activation_min_charge_for(80, 0.0), 1);
    }

    #[test]
    fn min_charge_full_ratio_returns_charge_max() {
        assert_eq!(activation_min_charge_for(80, 1.0), 80);
    }

    #[test]
    fn min_charge_zero_max_returns_one() {
        assert_eq!(activation_min_charge_for(0, 0.05), 1);
    }

    // scale_duration

    #[test]
    fn scale_full_charge_is_identity() {
        assert_eq!(scale_duration(5_000, 80, 80), 5_000);
    }

    #[test]
    fn scale_half_charge_halves_duration() {
        assert_eq!(scale_duration(5_000, 40, 80), 2_500);
    }

    #[test]
    fn scale_one_unit_minimum_never_zero() {
        // smallest non-zero charge over a large max: floor rounds to 0, clamped to 1
        assert_eq!(scale_duration(5_000, 1, 10_000), 1);
    }

    #[test]
    fn scale_zero_charge_max_returns_full_duration() {
        assert_eq!(scale_duration(5_000, 0, 0), 5_000);
    }

    #[test]
    fn scale_quarter_charge_rounds_down() {
        // 5000 * 20 / 80 = 1250
        assert_eq!(scale_duration(5_000, 20, 80), 1_250);
    }

    // residual_charge

    #[test]
    fn residual_at_start_equals_start_charge() {
        // remaining == duration → no time elapsed
        assert_eq!(residual_charge(80, 5_000, 5_000), 80);
    }

    #[test]
    fn residual_at_half_time() {
        assert_eq!(residual_charge(80, 2_500, 5_000), 40);
    }

    #[test]
    fn residual_at_zero_remaining() {
        assert_eq!(residual_charge(80, 0, 5_000), 0);
    }

    #[test]
    fn residual_zero_duration_returns_zero() {
        assert_eq!(residual_charge(80, 0, 0), 0);
    }

    #[test]
    fn residual_clamped_when_remaining_exceeds_duration() {
        // remaining > duration should behave as if remaining == duration
        assert_eq!(residual_charge(80, 9_999, 5_000), 80);
    }

    #[test]
    fn residual_proportional_to_start_charge() {
        // Same time ratio, different start charges → proportional residual
        let r40 = residual_charge(40, 2_500, 5_000);
        let r80 = residual_charge(80, 2_500, 5_000);
        assert_eq!(r80, r40 * 2);
    }
}
