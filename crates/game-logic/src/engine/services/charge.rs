/// Apply a scoring event to the charge system.
///
/// `base_pts` is the raw score before multipliers.
/// `weight` is the per-source, per-character coefficient.
/// Returns `(new_charge, new_buffer)`.
pub fn score_to_charge(
    base_pts: u64,
    weight: f32,
    buffer: u32,
    charge_ratio: u32,
    current_charge: u32,
    charge_max: u32,
) -> (u32, u32) {
    let weighted = (base_pts as f32 * weight).round() as u32;
    let buffer = buffer.saturating_add(weighted);
    let gain = buffer / charge_ratio;
    let new_buffer = buffer % charge_ratio;
    let new_charge = current_charge.saturating_add(gain).min(charge_max);
    (new_charge, new_buffer)
}

/// Accumulate time-based charge for one tick of `delta_s` seconds.
///
/// Returns `(gain, new_buffer)`. Gain is the integer charge units earned;
/// the fractional remainder is carried in `new_buffer` for the next tick.
pub fn time_to_charge(time_rate: f32, delta_s: f32, buffer: f32) -> (u32, f32) {
    let new_buffer = buffer + time_rate * delta_s;
    let gain = new_buffer.floor() as u32;
    (gain, new_buffer - gain as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    //score_to_charge

    #[test]
    fn one_hit_at_ratio_gives_one_unit() {
        // 100 pts, weight 1.0, ratio 100 → gain 1
        let (charge, buf) = score_to_charge(100, 1.0, 0, 100, 0, 80);
        assert_eq!(charge, 1);
        assert_eq!(buf, 0);
    }

    #[test]
    fn partial_hit_accumulates_in_buffer() {
        // 50 pts → 50 in buffer, no gain yet
        let (charge, buf) = score_to_charge(50, 1.0, 0, 100, 0, 80);
        assert_eq!(charge, 0);
        assert_eq!(buf, 50);
    }

    #[test]
    fn buffer_carries_over_between_hits() {
        // 50 pts buffer from before + 50 pts → 100 total → gain 1
        let (charge, buf) = score_to_charge(50, 1.0, 50, 100, 0, 80);
        assert_eq!(charge, 1);
        assert_eq!(buf, 0);
    }

    #[test]
    fn charge_capped_at_max() {
        // Huge points should not exceed charge_max
        let (charge, _) = score_to_charge(100_000, 1.0, 0, 100, 79, 80);
        assert_eq!(charge, 80);
    }

    #[test]
    fn weight_below_one_reduces_gain() {
        // weight 0.3, 100 pts → 30 weighted, below ratio 100 → no gain, buffer 30
        let (charge, buf) = score_to_charge(100, 0.3, 0, 100, 0, 80);
        assert_eq!(charge, 0);
        assert_eq!(buf, 30);
    }

    #[test]
    fn weight_above_one_amplifies_gain() {
        // weight 2.0, 100 pts → 200 weighted → gain 2
        let (charge, buf) = score_to_charge(100, 2.0, 0, 100, 0, 80);
        assert_eq!(charge, 2);
        assert_eq!(buf, 0);
    }

    #[test]
    fn zero_weight_produces_no_gain() {
        let (charge, buf) = score_to_charge(10_000, 0.0, 0, 100, 0, 80);
        assert_eq!(charge, 0);
        assert_eq!(buf, 0);
    }

    #[test]
    fn existing_charge_preserved_on_no_gain() {
        let (charge, _) = score_to_charge(10, 0.0, 0, 100, 42, 80);
        assert_eq!(charge, 42);
    }

    // time_to_charge

    #[test]
    fn one_second_at_rate_one_gives_one_unit() {
        let (gain, buf) = time_to_charge(1.0, 1.0, 0.0);
        assert_eq!(gain, 1);
        assert!((buf - 0.0).abs() < 1e-6);
    }

    #[test]
    fn sub_second_tick_accumulates_in_buffer() {
        // 250 ms tick (pve_tick_interval_ms = 250), rate 1.0 → 0.25 per tick
        let (gain, buf) = time_to_charge(1.0, 0.25, 0.0);
        assert_eq!(gain, 0);
        assert!((buf - 0.25).abs() < 1e-6);
    }

    #[test]
    fn four_ticks_of_250ms_give_one_unit() {
        let mut buf = 0.0f32;
        let mut total_gain = 0u32;
        for _ in 0..4 {
            let (g, b) = time_to_charge(1.0, 0.25, buf);
            total_gain += g;
            buf = b;
        }
        assert_eq!(total_gain, 1);
        assert!(buf.abs() < 1e-6);
    }

    #[test]
    fn zero_rate_produces_no_gain() {
        let (gain, buf) = time_to_charge(0.0, 1.0, 0.0);
        assert_eq!(gain, 0);
        assert!((buf - 0.0).abs() < 1e-6);
    }

    #[test]
    fn high_rate_gives_multiple_units_per_tick() {
        // rate 4.0, delta 1.0s → 4 units
        let (gain, buf) = time_to_charge(4.0, 1.0, 0.0);
        assert_eq!(gain, 4);
        assert!(buf.abs() < 1e-6);
    }

    #[test]
    fn existing_buffer_adds_to_new_accumulation() {
        // Start with 0.9 in buffer, add 0.25 → 1.15 → gain 1, buf 0.15
        let (gain, buf) = time_to_charge(1.0, 0.25, 0.9);
        assert_eq!(gain, 1);
        assert!((buf - 0.15).abs() < 1e-5);
    }
}
