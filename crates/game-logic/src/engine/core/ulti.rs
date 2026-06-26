use std::time::{Duration, Instant};

use shared::screen::{ScreenEnvelope, ScreenEventType};

use crate::engine::config;
use crate::player::personnages::character::UltiShape;

use super::{GameEngine, GHOST_CYCLE, make_event_envelope};

impl GameEngine {
    /// Expire a sustained ulti that has run its full duration.
    /// Called at the top of every `process` to avoid needing a background timer.
    pub(super) fn try_expire_ulti(&mut self, now: Instant) {
        if let Some(ends_at) = self.state.ulti_ends_at
            && now >= ends_at
            && self.state.ulti_active_id.is_some()
        {
            self.state.ulti_ends_at = None;
            self.state.ultimate_charge = 0;
            self.state.ulti_active_id = None;
            self.state.ulti_multiplier_override = None;
        }
    }

    /// Handle an L2/R2 press: activates the ulti if charge is full, or cancels
    /// it if a cancellable ulti is already running.
    pub(super) fn process_ulti_press(&mut self, now: Instant) -> Vec<ScreenEnvelope> {
        let mut envelopes = Vec::new();

        if self.state.is_ulti_active(now) {
            if self.state.ulti_cancellable {
                let charge_max = self.character.stats().charge_profile.charge_max;
                let residual = self.state.residual_charge_with_max(now, charge_max);
                let ulti_id = self.state.ulti_active_id.clone().unwrap_or_default();

                self.state.ulti_ends_at = Some(now);
                self.state.ultimate_charge = residual;
                self.state.ulti_active_id = None;
                self.state.ulti_multiplier_override = None;

                envelopes.push(make_event_envelope(
                    ScreenEventType::UltimateStopped,
                    serde_json::json!({
                        "ulti_id": ulti_id,
                        "ultimate_charge": residual,
                    }),
                ));
                envelopes.push(self.emit_score_update(None));
            }
        } else {
            let charge_max = self.character.stats().charge_profile.charge_max;
            if self.state.ultimate_charge >= charge_max {
                envelopes.extend(self.activate_ulti(now));
                envelopes.push(self.emit_score_update(None));
            }
        }

        envelopes
    }

    fn activate_ulti(&mut self, now: Instant) -> Vec<ScreenEnvelope> {
        let mut envelopes = Vec::new();
        let character_slug = self.character.slug();
        let cfg = config::get();

        let (ulti_id, shape): (&'static str, UltiShape) = if character_slug == "ghost" {
            let idx = (self.state.ghost_cycle_index as usize) % GHOST_CYCLE.len();
            self.state.ghost_cycle_index = self.state.ghost_cycle_index.wrapping_add(1);
            match idx {
                0 => ("multiball_split", UltiShape::Instant),
                1 => (
                    "rampage",
                    UltiShape::Sustained {
                        duration_ms: cfg.viper_ulti_duration_ms,
                        cancellable: false,
                    },
                ),
                _ => (
                    "time_slow",
                    UltiShape::Sustained {
                        duration_ms: cfg.oracle_ulti_duration_ms,
                        cancellable: true,
                    },
                ),
            }
        } else {
            (self.character.ulti_id(), self.character.ulti_shape())
        };

        match &shape {
            UltiShape::Instant => {
                self.state.ultimate_charge = 0;
                if ulti_id == "multiball_split" {
                    self.state.multiball_active = true;
                }
            }
            UltiShape::Sustained {
                duration_ms,
                cancellable,
            } => {
                self.state.ulti_ends_at = Some(now + Duration::from_millis(*duration_ms));
                self.state.ulti_duration_ms = *duration_ms;
                self.state.ulti_cancellable = *cancellable;
                self.state.ulti_active_id = Some(ulti_id.to_string());

                if ulti_id == "rampage" {
                    let rampage_mult = cfg.viper_rampage_multiplier;
                    self.state.ulti_multiplier_override = Some(rampage_mult);
                    envelopes.push(make_event_envelope(
                        ScreenEventType::MultiplierUpdate,
                        serde_json::json!({
                            "multiplier": rampage_mult,
                            "duration_ms": duration_ms,
                        }),
                    ));
                }
            }
            UltiShape::Inherited => {
                unreachable!("ghost always resolves to a concrete shape before this point");
            }
        }

        let mut triggered = serde_json::json!({
            "character": character_slug,
            "ulti_id": ulti_id,
        });
        match &shape {
            UltiShape::Instant => {
                triggered["shape"] = serde_json::json!("instant");
                triggered["cancellable"] = serde_json::json!(false);
            }
            UltiShape::Sustained {
                duration_ms,
                cancellable,
            } => {
                triggered["shape"] = serde_json::json!("sustained");
                triggered["cancellable"] = serde_json::json!(cancellable);
                triggered["duration_ms"] = serde_json::json!(duration_ms);
                let payload = match ulti_id {
                    "rampage" => serde_json::json!({ "multiplier": cfg.viper_rampage_multiplier }),
                    "time_slow" => serde_json::json!({ "slow_factor": cfg.oracle_slow_factor }),
                    _ => serde_json::json!({}),
                };
                triggered["payload"] = payload;
            }
            UltiShape::Inherited => unreachable!(),
        }
        envelopes.push(make_event_envelope(
            ScreenEventType::UltimateTriggered,
            triggered,
        ));

        envelopes
    }
}
