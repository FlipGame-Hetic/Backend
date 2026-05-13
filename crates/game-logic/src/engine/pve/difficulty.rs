use crate::engine::config::{
    BOSS_0_DIFFICULTY_SCALE, BOSS_1_DIFFICULTY_SCALE, BOSS_2_DIFFICULTY_SCALE,
    ENDLESS_BASE_DIFFICULTY_SCALE, ENDLESS_LEVEL_SCALE_EXPONENT,
};

pub fn scale_hp(base_hp: u32, boss_index: u8, endless_level: u32) -> u32 {
    let factor = match boss_index {
        0 => BOSS_0_DIFFICULTY_SCALE,
        1 => BOSS_1_DIFFICULTY_SCALE,
        2 => BOSS_2_DIFFICULTY_SCALE,
        _ => {
            let endless_factor = ENDLESS_BASE_DIFFICULTY_SCALE
                * ENDLESS_LEVEL_SCALE_EXPONENT.powi(endless_level as i32);
            return (base_hp as f32 * endless_factor) as u32;
        }
    };
    (base_hp as f32 * factor) as u32
}

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
