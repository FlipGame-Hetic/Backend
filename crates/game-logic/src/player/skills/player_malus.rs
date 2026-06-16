use shared::screen::ScreenEventType;

use crate::player::skills::player_bonus::SkillEffect;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameMode {
    Pve,
    Pvp,
}

#[derive(Debug, Clone, Copy)]
pub enum MalusSkill {
    Invisible,
    InkBlot,
    BumperReduction,
    BlackHole,
    ModifyBounce,
    StickyBumpers,
}

impl MalusSkill {
    pub fn activate(&self, mode: GameMode) -> SkillEffect {
        if mode == GameMode::Pve {
            return SkillEffect::NoEffect;
        }
        // PvP: target opponent via screen event
        let event_type = match self {
            Self::Invisible => ScreenEventType::MalusInvisible,
            Self::InkBlot => ScreenEventType::MalusInkBlot,
            Self::BumperReduction => ScreenEventType::MalusBumperReduction,
            Self::BlackHole => ScreenEventType::MalusBlackHole,
            Self::ModifyBounce => ScreenEventType::MalusModifyBounce,
            Self::StickyBumpers => ScreenEventType::MalusStickyBumpers,
        };
        SkillEffect::EmitScreenEvent {
            event_type,
            payload: serde_json::json!({ "target": "opponent" }),
        }
    }
}
