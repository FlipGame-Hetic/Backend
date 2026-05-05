use crate::engine::config::{
    CYBORG_BONUS_COOLDOWN_MS, CYBORG_MALUS_COOLDOWN_MS, CYBORG_ULTIMATE_MAX,
    DREDD_BONUS_COOLDOWN_MS, DREDD_MALUS_COOLDOWN_MS, DREDD_ULTIMATE_MAX, HACKER_BONUS_COOLDOWN_MS,
    HACKER_MALUS_COOLDOWN_MS, HACKER_ULTIMATE_MAX, ROBOCP_BONUS_COOLDOWN_MS,
    ROBOCP_MALUS_COOLDOWN_MS, ROBOCP_ULTIMATE_MAX,
};
use crate::player::personnages::character_stats::CharacterStats;
use crate::player::skills::{BonusSkill, MalusSkill};

pub trait Character: Send + Sync {
    fn id(&self) -> u8;
    fn name(&self) -> &'static str;
    fn stats(&self) -> &CharacterStats;
    fn bonus(&self) -> BonusSkill;
    fn malus(&self) -> MalusSkill;
}

pub struct RoboCop;
pub struct JudgeDredd;
pub struct Hacker;
pub struct Cyborg;

impl Character for RoboCop {
    fn id(&self) -> u8 { 0 }
    fn name(&self) -> &'static str { "RoboCop" }
    fn stats(&self) -> &CharacterStats {
        static STATS: CharacterStats = CharacterStats {
            ultimate_charge_max: ROBOCP_ULTIMATE_MAX,
            bonus_cooldown_ms: ROBOCP_BONUS_COOLDOWN_MS,
            malus_cooldown_ms: ROBOCP_MALUS_COOLDOWN_MS,
        };
        &STATS
    }
    fn bonus(&self) -> BonusSkill { BonusSkill::Shield }
    fn malus(&self) -> MalusSkill { MalusSkill::InkBlot }
}

impl Character for JudgeDredd {
    fn id(&self) -> u8 { 1 }
    fn name(&self) -> &'static str { "Judge Dredd" }
    fn stats(&self) -> &CharacterStats {
        static STATS: CharacterStats = CharacterStats {
            ultimate_charge_max: DREDD_ULTIMATE_MAX,
            bonus_cooldown_ms: DREDD_BONUS_COOLDOWN_MS,
            malus_cooldown_ms: DREDD_MALUS_COOLDOWN_MS,
        };
        &STATS
    }
    fn bonus(&self) -> BonusSkill { BonusSkill::DamageBoost }
    fn malus(&self) -> MalusSkill { MalusSkill::BumperReduction }
}

impl Character for Hacker {
    fn id(&self) -> u8 { 2 }
    fn name(&self) -> &'static str { "Hacker" }
    fn stats(&self) -> &CharacterStats {
        static STATS: CharacterStats = CharacterStats {
            ultimate_charge_max: HACKER_ULTIMATE_MAX,
            bonus_cooldown_ms: HACKER_BONUS_COOLDOWN_MS,
            malus_cooldown_ms: HACKER_MALUS_COOLDOWN_MS,
        };
        &STATS
    }
    fn bonus(&self) -> BonusSkill { BonusSkill::ComboMultiplier }
    fn malus(&self) -> MalusSkill { MalusSkill::Invisible }
}

impl Character for Cyborg {
    fn id(&self) -> u8 { 3 }
    fn name(&self) -> &'static str { "Cyborg" }
    fn stats(&self) -> &CharacterStats {
        static STATS: CharacterStats = CharacterStats {
            ultimate_charge_max: CYBORG_ULTIMATE_MAX,
            bonus_cooldown_ms: CYBORG_BONUS_COOLDOWN_MS,
            malus_cooldown_ms: CYBORG_MALUS_COOLDOWN_MS,
        };
        &STATS
    }
    fn bonus(&self) -> BonusSkill { BonusSkill::ExtraFlippers }
    fn malus(&self) -> MalusSkill { MalusSkill::ModifyBounce }
}

pub fn select_character(id: u8) -> Box<dyn Character> {
    match id {
        0 => Box::new(RoboCop),
        1 => Box::new(JudgeDredd),
        2 => Box::new(Hacker),
        3 => Box::new(Cyborg),
        unknown => {
            tracing::warn!(character_id = unknown, "unknown character id, defaulting to RoboCop");
            Box::new(RoboCop)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_select_character_robocp() {
        let c = select_character(0);
        assert_eq!(c.id(), 0);
        assert_eq!(c.name(), "RoboCop");
    }

    #[test]
    fn test_select_invalid_id_defaults() {
        let c = select_character(99);
        assert_eq!(c.id(), 0);
    }

    #[test]
    fn test_all_characters_have_unique_ids() {
        let ids: Vec<u8> = vec![
            select_character(0).id(),
            select_character(1).id(),
            select_character(2).id(),
            select_character(3).id(),
        ];
        let mut sorted = ids.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), ids.len());
    }
}
