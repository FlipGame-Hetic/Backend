use crate::engine::config::{
    ENFORCER_CHARGE_MAX, ENFORCER_WEIGHT_BUMPER, ENFORCER_WEIGHT_COMBO, ENFORCER_WEIGHT_OTHER,
    ENFORCER_WEIGHT_RAIL, GHOST_CHARGE_MAX, ORACLE_CHARGE_MAX, ORACLE_TIME_RATE,
    ORACLE_ULTI_DURATION_MS, VIPER_CHARGE_MAX, VIPER_ULTI_DURATION_MS,
};
use crate::player::personnages::character_stats::{CharacterChargeProfile, CharacterStats};

#[derive(Debug, Clone)]
pub enum UltiShape {
    Instant,
    Sustained {
        duration_ms: u64,
        cancellable: bool,
    },
    /// Ghost inherits from the cycle; resolved at activation time.
    Inherited,
}

pub trait Character: Send + Sync {
    fn slug(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn stats(&self) -> &CharacterStats;
    fn ulti_id(&self) -> &'static str;
    fn ulti_shape(&self) -> UltiShape;
}

pub struct Enforcer;
pub struct Viper;
pub struct Ghost;
pub struct Oracle;

impl Character for Enforcer {
    fn slug(&self) -> &'static str {
        "enforcer"
    }
    fn name(&self) -> &'static str {
        "KEENU"
    }
    fn stats(&self) -> &CharacterStats {
        static STATS: CharacterStats = CharacterStats {
            charge_profile: CharacterChargeProfile {
                charge_max: ENFORCER_CHARGE_MAX,
                weight_bumper: ENFORCER_WEIGHT_BUMPER,
                weight_rail: ENFORCER_WEIGHT_RAIL,
                weight_combo: ENFORCER_WEIGHT_COMBO,
                weight_other: ENFORCER_WEIGHT_OTHER,
                time_rate: 0.0,
            },
        };
        &STATS
    }
    fn ulti_id(&self) -> &'static str {
        "multiball_split"
    }
    fn ulti_shape(&self) -> UltiShape {
        UltiShape::Instant
    }
}

impl Character for Viper {
    fn slug(&self) -> &'static str {
        "viper"
    }
    fn name(&self) -> &'static str {
        "VIPER"
    }
    fn stats(&self) -> &CharacterStats {
        static STATS: CharacterStats = CharacterStats {
            charge_profile: CharacterChargeProfile {
                charge_max: VIPER_CHARGE_MAX,
                weight_bumper: 1.0,
                weight_rail: 1.0,
                weight_combo: 1.0,
                weight_other: 1.0,
                time_rate: 0.0,
            },
        };
        &STATS
    }
    fn ulti_id(&self) -> &'static str {
        "rampage"
    }
    fn ulti_shape(&self) -> UltiShape {
        UltiShape::Sustained {
            duration_ms: VIPER_ULTI_DURATION_MS,
            cancellable: false,
        }
    }
}

impl Character for Ghost {
    fn slug(&self) -> &'static str {
        "ghost"
    }
    fn name(&self) -> &'static str {
        "GHOST"
    }
    fn stats(&self) -> &CharacterStats {
        static STATS: CharacterStats = CharacterStats {
            charge_profile: CharacterChargeProfile {
                charge_max: GHOST_CHARGE_MAX,
                weight_bumper: 1.0,
                weight_rail: 1.0,
                weight_combo: 1.0,
                weight_other: 1.0,
                time_rate: 0.0,
            },
        };
        &STATS
    }
    fn ulti_id(&self) -> &'static str {
        "mimic"
    }
    fn ulti_shape(&self) -> UltiShape {
        UltiShape::Inherited
    }
}

impl Character for Oracle {
    fn slug(&self) -> &'static str {
        "oracle"
    }
    fn name(&self) -> &'static str {
        "ORACLE"
    }
    fn stats(&self) -> &CharacterStats {
        static STATS: CharacterStats = CharacterStats {
            charge_profile: CharacterChargeProfile {
                charge_max: ORACLE_CHARGE_MAX,
                weight_bumper: 1.0,
                weight_rail: 1.0,
                weight_combo: 1.0,
                weight_other: 1.0,
                time_rate: ORACLE_TIME_RATE,
            },
        };
        &STATS
    }
    fn ulti_id(&self) -> &'static str {
        "time_slow"
    }
    fn ulti_shape(&self) -> UltiShape {
        UltiShape::Sustained {
            duration_ms: ORACLE_ULTI_DURATION_MS,
            cancellable: true,
        }
    }
}

pub fn select_character(slug: &str) -> Box<dyn Character> {
    match slug {
        "enforcer" => Box::new(Enforcer),
        "viper" => Box::new(Viper),
        "ghost" => Box::new(Ghost),
        "oracle" => Box::new(Oracle),
        unknown => {
            tracing::warn!(
                character = unknown,
                "unknown character slug, defaulting to enforcer"
            );
            Box::new(Enforcer)
        }
    }
}

/// Maps a character slug to a stable integer for DB persistence.
pub fn slug_to_db_id(slug: &str) -> u8 {
    match slug {
        "enforcer" => 0,
        "viper" => 1,
        "ghost" => 2,
        "oracle" => 3,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_select_character_enforcer() {
        let c = select_character("enforcer");
        assert_eq!(c.slug(), "enforcer");
        assert_eq!(c.name(), "KEENU");
    }

    #[test]
    fn test_select_invalid_slug_defaults() {
        let c = select_character("unknown_hero");
        assert_eq!(c.slug(), "enforcer");
    }

    #[test]
    fn test_all_characters_have_unique_slugs() {
        let slugs = ["enforcer", "viper", "ghost", "oracle"];
        let mut seen: Vec<&str> = slugs.iter().map(|s| select_character(s).slug()).collect();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), slugs.len());
    }

    #[test]
    fn test_slug_to_db_id_unique() {
        let ids: Vec<u8> = ["enforcer", "viper", "ghost", "oracle"]
            .iter()
            .map(|s| slug_to_db_id(s))
            .collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), ids.len());
    }
}
