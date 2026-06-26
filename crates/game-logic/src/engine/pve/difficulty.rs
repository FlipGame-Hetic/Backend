//! Boss HP scaling formulas — translates config difficulty factors into actual HP values.

use crate::engine::config;

/// Scale `base_hp` by the difficulty factor for `boss_index`.
/// For endless bosses (index ≥ 3) HP grows exponentially with `endless_level`.
pub fn scale_hp(base_hp: u32, boss_index: u8, endless_level: u32) -> u32 {
    let cfg = config::get();
    let factor = match boss_index {
        0 => cfg.boss_0_difficulty_scale,
        1 => cfg.boss_1_difficulty_scale,
        2 => cfg.boss_2_difficulty_scale,
        _ => {
            let endless_factor = cfg.endless_base_difficulty_scale
                * cfg.endless_level_scale_exponent.powi(endless_level as i32);
            return (base_hp as f32 * endless_factor) as u32;
        }
    };
    (base_hp as f32 * factor) as u32
}

/// Convert player score points into boss HP damage.
/// Currently 1:1 — kept as a separate function so the formula can be tuned later.
pub fn boss_damage_to_health(bumper_pts: u32, _boss_index: u8) -> u32 {
    bumper_pts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scale_boss_0() {
        assert_eq!(scale_hp(500, 0, 0), 500);
    }

    #[test]
    fn test_scale_boss_1() {
        assert_eq!(scale_hp(500, 1, 0), 800);
    }

    #[test]
    fn test_scale_boss_2() {
        assert_eq!(scale_hp(500, 2, 0), 1_200);
    }

    #[test]
    fn test_scale_endless() {
        let hp = scale_hp(500, 3, 1);
        // 500 * 2.4 * 1.3 = 1560
        assert_eq!(hp, 1_560);
    }
}
