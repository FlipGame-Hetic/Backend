use std::time::Instant;

use shared::screen::{ScreenEnvelope, ScreenEventType};

use crate::combo::ComboEffect;

use super::{GHOST_CYCLE, GameEngine, make_event_envelope};

impl GameEngine {
    pub(super) fn emit_score_update(&self, ball_id: Option<String>) -> ScreenEnvelope {
        let now = Instant::now();
        let current_multiplier = self.effective_multiplier(now);
        let ball = self.state.balls_lost_since_start + 1;
        let charge_max = self.character.stats().charge_profile.charge_max;

        let displayed_charge = if self.state.is_ulti_active(now) {
            self.state.residual_charge(now)
        } else {
            self.state.ultimate_charge
        };
        let ulti_ready = !self.state.is_ulti_active(now)
            && self.state.ultimate_charge >= self.activation_min_charge();

        let mut payload = serde_json::json!({
            "score": self.state.score,
            "multiplier": current_multiplier,
            "ball": ball,
            "ultimate_charge": displayed_charge,
            "ultimate_max": charge_max,
            "ulti_ready": ulti_ready,
        });

        if self.character.slug() == "ghost" {
            let next_idx = (self.state.ghost_cycle_index as usize) % GHOST_CYCLE.len();
            payload["next_ulti_id"] = serde_json::json!(GHOST_CYCLE[next_idx]);
        }

        if let Some(bid) = ball_id {
            payload["ball_id"] = serde_json::json!(bid);
        }
        make_event_envelope(ScreenEventType::ScoreUpdate, payload)
    }

    pub(super) fn emit_score_delta(&self, delta: u64, reason: &str) -> ScreenEnvelope {
        self.emit_scored_delta(delta, reason, None)
    }

    pub(super) fn emit_scored_delta(
        &self,
        delta: u64,
        reason: &str,
        ball_id: Option<String>,
    ) -> ScreenEnvelope {
        let mut payload = serde_json::json!({
            "delta": delta,
            "reason": reason,
            "total": self.state.score,
        });
        if let Some(bid) = ball_id {
            payload["ball_id"] = serde_json::json!(bid);
        }
        make_event_envelope(ScreenEventType::ScoreDelta, payload)
    }

    pub(super) fn emit_life_update(&self) -> ScreenEnvelope {
        make_event_envelope(
            ScreenEventType::LifeUpdate,
            serde_json::json!({ "lives_remaining": self.state.lives }),
        )
    }

    pub(super) fn emit_combo_activated(&self, effect: &ComboEffect) -> ScreenEnvelope {
        make_event_envelope(
            ScreenEventType::ComboActivated,
            serde_json::json!({
                "bonus_pts": effect.bonus_pts,
                "sequence": effect.sequence,
            }),
        )
    }

    pub(super) fn emit_game_over(&self) -> ScreenEnvelope {
        make_event_envelope(
            ScreenEventType::GameOver,
            serde_json::json!({ "final_score": self.state.score }),
        )
    }
}
