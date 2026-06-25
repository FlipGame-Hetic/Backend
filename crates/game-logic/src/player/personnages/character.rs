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
    fn stats(&self) -> CharacterStats;
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
    fn stats(&self) -> CharacterStats {
        let cfg = crate::engine::config::get();
        CharacterStats {
            charge_profile: CharacterChargeProfile {
                charge_max: cfg.enforcer_charge_max,
                weight_bumper: cfg.enforcer_weight_bumper,
                weight_rail: cfg.enforcer_weight_rail,
                weight_combo: cfg.enforcer_weight_combo,
                weight_other: cfg.enforcer_weight_other,
                time_rate: 0.0,
            },
        }
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
    fn stats(&self) -> CharacterStats {
        let cfg = crate::engine::config::get();
        CharacterStats {
            charge_profile: CharacterChargeProfile {
                charge_max: cfg.viper_charge_max,
                weight_bumper: 1.0,
                weight_rail: 1.0,
                weight_combo: 1.0,
                weight_other: 1.0,
                time_rate: 0.0,
            },
        }
    }
    fn ulti_id(&self) -> &'static str {
        "rampage"
    }
    fn ulti_shape(&self) -> UltiShape {
        UltiShape::Sustained {
            duration_ms: crate::engine::config::get().viper_ulti_duration_ms,
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
    fn stats(&self) -> CharacterStats {
        let cfg = crate::engine::config::get();
        CharacterStats {
            charge_profile: CharacterChargeProfile {
                charge_max: cfg.ghost_charge_max,
                weight_bumper: 1.0,
                weight_rail: 1.0,
                weight_combo: 1.0,
                weight_other: 1.0,
                time_rate: 0.0,
            },
        }
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
    fn stats(&self) -> CharacterStats {
        let cfg = crate::engine::config::get();
        CharacterStats {
            charge_profile: CharacterChargeProfile {
                charge_max: cfg.oracle_charge_max,
                weight_bumper: 1.0,
                weight_rail: 1.0,
                weight_combo: 1.0,
                weight_other: 1.0,
                time_rate: cfg.oracle_time_rate,
            },
        }
    }
    fn ulti_id(&self) -> &'static str {
        "time_slow"
    }
    fn ulti_shape(&self) -> UltiShape {
        UltiShape::Sustained {
            duration_ms: crate::engine::config::get().oracle_ulti_duration_ms,
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
