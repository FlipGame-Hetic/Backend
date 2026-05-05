use std::time::{Duration, Instant};

use crate::engine::config::{
    SKILL_COMBO_MULTIPLIER_DURATION_MS, SKILL_COMBO_MULTIPLIER_FACTOR, SKILL_DAMAGE_BOOST_DURATION_MS,
    SKILL_DAMAGE_BOOST_MULTIPLIER, SKILL_EXTRA_FLIPPERS_DURATION_MS, SKILL_FREEZE_DURATION_MS,
    SKILL_PORTAL_BONUS_PTS, SKILL_SHIELD_DURATION_MS, SKILL_TIME_SLOWDOWN_DURATION_MS,
};
use crate::engine::states::GameState;

#[derive(Debug, Clone)]
pub enum SkillEffect {
    ModifyMultiplier { factor: f32, duration_ms: u64 },
    AddBalls { count: u8 },
    ShieldActivated { duration_ms: u64 },
    AddScore { pts: u32 },
    EmitScreenEvent { event_type: String, payload: serde_json::Value },
    NoEffect,
}

#[derive(Debug, Clone, Copy)]
pub enum BonusSkill {
    Shield,
    TimeSlowdown,
    ComboMultiplier,
    DamageBoost,
    ExtraFlippers,
    Portal,
    Freeze,
    ExtraBall,
}

impl BonusSkill {
    pub fn activate(&self, state: &mut GameState) -> SkillEffect {
        let now = Instant::now();
        match self {
            Self::Shield => {
                state.shield_active = true;
                state.shield_expires_at = Some(now + Duration::from_millis(SKILL_SHIELD_DURATION_MS));
                SkillEffect::ShieldActivated { duration_ms: SKILL_SHIELD_DURATION_MS }
            }
            Self::DamageBoost => {
                state.damage_multiplier = SKILL_DAMAGE_BOOST_MULTIPLIER;
                state.active_multiplier = SKILL_DAMAGE_BOOST_MULTIPLIER;
                state.multiplier_expires_at =
                    Some(now + Duration::from_millis(SKILL_DAMAGE_BOOST_DURATION_MS));
                SkillEffect::ModifyMultiplier {
                    factor: SKILL_DAMAGE_BOOST_MULTIPLIER,
                    duration_ms: SKILL_DAMAGE_BOOST_DURATION_MS,
                }
            }
            Self::ComboMultiplier => {
                state.active_multiplier = SKILL_COMBO_MULTIPLIER_FACTOR;
                state.multiplier_expires_at =
                    Some(now + Duration::from_millis(SKILL_COMBO_MULTIPLIER_DURATION_MS));
                SkillEffect::ModifyMultiplier {
                    factor: SKILL_COMBO_MULTIPLIER_FACTOR,
                    duration_ms: SKILL_COMBO_MULTIPLIER_DURATION_MS,
                }
            }
            Self::ExtraFlippers => SkillEffect::EmitScreenEvent {
                event_type: "ExtraFlippers".to_owned(),
                payload: serde_json::json!({ "duration_ms": SKILL_EXTRA_FLIPPERS_DURATION_MS }),
            },
            Self::ExtraBall => {
                state.extra_balls = state.extra_balls.saturating_add(1);
                SkillEffect::AddBalls { count: 1 }
            }
            Self::Portal => SkillEffect::AddScore { pts: SKILL_PORTAL_BONUS_PTS },
            Self::TimeSlowdown => SkillEffect::EmitScreenEvent {
                event_type: "TimeSlowdown".to_owned(),
                payload: serde_json::json!({ "duration_ms": SKILL_TIME_SLOWDOWN_DURATION_MS }),
            },
            Self::Freeze => SkillEffect::EmitScreenEvent {
                event_type: "Freeze".to_owned(),
                payload: serde_json::json!({ "duration_ms": SKILL_FREEZE_DURATION_MS }),
            },
        }
    }
}
