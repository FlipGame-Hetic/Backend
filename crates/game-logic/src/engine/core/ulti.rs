use std::time::{Duration, Instant};

use shared::screen::{ScreenEnvelope, ScreenEventType};

use crate::engine::config;
use crate::player::personnages::character::UltiShape;

use super::{GHOST_CYCLE, GameEngine, make_event_envelope};

/// Minimum charge required to activate a time_slow ulti at `ratio` of `charge_max`.
/// Always returns at least 1 to avoid a zero threshold.
pub(crate) fn activation_min_charge_for(charge_max: u32, ratio: f32) -> u32 {
    ((charge_max as f32 * ratio).ceil() as u32).max(1)
}

impl GameEngine {
    /// Returns the ulti_id Ghost's next cycle would produce without advancing the index.
    fn peek_ghost_ulti_id(&self) -> &'static str {
        GHOST_CYCLE[(self.state.ghost_cycle_index as usize) % GHOST_CYCLE.len()]
    }

    /// True when the next ulti to activate resolves to `time_slow` (Oracle or Ghost at that slot).
    fn next_ulti_is_time_slow(&self) -> bool {
        match self.character.slug() {
            "oracle" => true,
            "ghost" => self.peek_ghost_ulti_id() == "time_slow",
            _ => false,
        }
    }

    /// Minimum charge required to fire the current character's next ulti.
    /// `time_slow` variants accept partial activation; all others require full charge.
    pub(super) fn activation_min_charge(&self) -> u32 {
        let charge_max = self.character.stats().charge_profile.charge_max;
        if self.next_ulti_is_time_slow() {
            activation_min_charge_for(charge_max, config::get().oracle_activation_min_ratio)
        } else {
            charge_max
        }
    }

    /// Expire a sustained ulti that has run its full duration.
    /// Called at the top of every `process` to avoid needing a background timer.
    pub(super) fn try_expire_ulti(&mut self, now: Instant) {
        if let Some(ends_at) = self.state.ulti_ends_at
            && now >= ends_at
            && self.state.ulti_active_id.is_some()
        {
            self.state.ulti_ends_at = None;
            self.state.ultimate_charge = 0;
            self.state.ulti_start_charge = 0;
            self.state.ulti_active_id = None;
            self.state.ulti_multiplier_override = None;
        }
    }

    /// Handle an L2/R2 press: activates the ulti when charge meets the threshold, or cancels
    /// a cancellable running ulti.
    pub(super) fn process_ulti_press(&mut self, now: Instant) -> Vec<ScreenEnvelope> {
        let mut envelopes = Vec::new();

        if self.state.is_ulti_active(now) {
            if self.state.ulti_cancellable {
                let residual = self.state.residual_charge(now);
                let ulti_id = self.state.ulti_active_id.clone().unwrap_or_default();

                self.state.ulti_ends_at = Some(now);
                self.state.ultimate_charge = residual;
                self.state.ulti_start_charge = 0;
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
            let min_charge = self.activation_min_charge();
            if self.state.ultimate_charge >= min_charge {
                let activation_charge = self.state.ultimate_charge;
                envelopes.extend(self.activate_ulti(now, activation_charge));
                envelopes.push(self.emit_score_update(None));
            }
        }

        envelopes
    }

    fn activate_ulti(&mut self, now: Instant, activation_charge: u32) -> Vec<ScreenEnvelope> {
        let mut envelopes = Vec::new();
        let character_slug = self.character.slug();
        let cfg = config::get();
        let charge_max = self.character.stats().charge_profile.charge_max;

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

        // Apply shape-specific state changes; compute the actual emission duration for Sustained.
        let emitted_duration_ms: u64 = match &shape {
            UltiShape::Instant => {
                self.state.ultimate_charge = 0;
                self.state.ulti_start_charge = 0;
                if ulti_id == "multiball_split" {
                    self.state.multiball_active = true;
                }
                0
            }
            UltiShape::Sustained {
                duration_ms: full_duration,
                cancellable,
            } => {
                // Scale duration proportionally to the committed charge.
                // At full charge the result is identical to the configured duration.
                let actual = if charge_max > 0 {
                    (*full_duration * activation_charge as u64 / charge_max as u64).max(1)
                } else {
                    *full_duration
                };
                self.state.ulti_ends_at = Some(now + Duration::from_millis(actual));
                self.state.ulti_duration_ms = actual;
                self.state.ulti_cancellable = *cancellable;
                self.state.ulti_active_id = Some(ulti_id.to_string());
                self.state.ulti_start_charge = activation_charge;
                self.state.ultimate_charge = 0;

                if ulti_id == "rampage" {
                    let rampage_mult = cfg.viper_rampage_multiplier;
                    self.state.ulti_multiplier_override = Some(rampage_mult);
                    envelopes.push(make_event_envelope(
                        ScreenEventType::MultiplierUpdate,
                        serde_json::json!({
                            "multiplier": rampage_mult,
                            "duration_ms": actual,
                        }),
                    ));
                }
                actual
            }
            UltiShape::Inherited => {
                unreachable!("ghost always resolves to a concrete shape before this point");
            }
        };

        let mut triggered = serde_json::json!({
            "character": character_slug,
            "ulti_id": ulti_id,
            "activation_charge": activation_charge,
        });
        match &shape {
            UltiShape::Instant => {
                triggered["shape"] = serde_json::json!("instant");
                triggered["cancellable"] = serde_json::json!(false);
            }
            UltiShape::Sustained { cancellable, .. } => {
                triggered["shape"] = serde_json::json!("sustained");
                triggered["cancellable"] = serde_json::json!(cancellable);
                triggered["duration_ms"] = serde_json::json!(emitted_duration_ms);
                triggered["payload"] = match ulti_id {
                    "rampage" => serde_json::json!({ "multiplier": cfg.viper_rampage_multiplier }),
                    "time_slow" => serde_json::json!({ "slow_factor": cfg.oracle_slow_factor }),
                    _ => serde_json::json!({}),
                };
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

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use shared::screen::ScreenEventType;

    use crate::engine::config;
    use crate::engine::events::GameEvent;

    use super::{GameEngine, activation_min_charge_for};

    fn oracle_engine() -> GameEngine {
        let mut e = GameEngine::new("oracle");
        e.process(GameEvent::StartGame);
        e
    }

    fn ghost_engine() -> GameEngine {
        let mut e = GameEngine::new("ghost");
        e.process(GameEvent::StartGame);
        e
    }

    // ── Oracle partial activation ──────────────────────────────────────────────

    #[test]
    fn oracle_does_not_trigger_below_threshold() {
        let mut engine = oracle_engine();
        let charge_max = engine.character.stats().charge_profile.charge_max;
        let min = activation_min_charge_for(charge_max, config::get().oracle_activation_min_ratio);
        if min == 0 {
            return;
        }
        engine.state.ultimate_charge = min - 1;
        let envelopes = engine.process_ulti_press(Instant::now());
        assert!(
            !envelopes
                .iter()
                .any(|e| e.event_type == ScreenEventType::UltimateTriggered),
            "should not trigger below threshold"
        );
        assert!(!engine.state.is_ulti_active(Instant::now()));
    }

    #[test]
    fn oracle_triggers_at_exact_threshold() {
        let mut engine = oracle_engine();
        let charge_max = engine.character.stats().charge_profile.charge_max;
        let min = activation_min_charge_for(charge_max, config::get().oracle_activation_min_ratio);
        engine.state.ultimate_charge = min;
        let envelopes = engine.process_ulti_press(Instant::now());
        assert!(
            envelopes
                .iter()
                .any(|e| e.event_type == ScreenEventType::UltimateTriggered),
            "should trigger at threshold"
        );
    }

    #[test]
    fn oracle_half_charge_sends_half_duration() {
        let mut engine = oracle_engine();
        let charge_max = engine.character.stats().charge_profile.charge_max;
        let half = charge_max / 2;
        engine.state.ultimate_charge = half;
        let full_duration = config::get().oracle_ulti_duration_ms;

        let envelopes = engine.process_ulti_press(Instant::now());
        let triggered = envelopes
            .iter()
            .find(|e| e.event_type == ScreenEventType::UltimateTriggered)
            .expect("should trigger");

        let emitted = triggered.payload["duration_ms"].as_u64().unwrap();
        let expected = full_duration * half as u64 / charge_max as u64;
        assert_eq!(emitted, expected);
    }

    #[test]
    fn oracle_full_charge_duration_unchanged() {
        let mut engine = oracle_engine();
        let charge_max = engine.character.stats().charge_profile.charge_max;
        engine.state.ultimate_charge = charge_max;
        let full_duration = config::get().oracle_ulti_duration_ms;

        let envelopes = engine.process_ulti_press(Instant::now());
        let triggered = envelopes
            .iter()
            .find(|e| e.event_type == ScreenEventType::UltimateTriggered)
            .expect("should trigger at full charge");
        assert_eq!(triggered.payload["duration_ms"].as_u64().unwrap(), full_duration);
    }

    #[test]
    fn oracle_cancel_residual_based_on_start_charge() {
        let mut engine = oracle_engine();
        let charge_max = engine.character.stats().charge_profile.charge_max;
        let start_charge = charge_max / 2;
        engine.state.ultimate_charge = start_charge;

        // Activate at half charge.
        engine.process_ulti_press(Instant::now());
        assert!(engine.state.is_ulti_active(Instant::now()));

        // Immediately cancel — residual should be close to start_charge.
        let envelopes = engine.process_ulti_press(Instant::now());
        let stopped = envelopes
            .iter()
            .find(|e| e.event_type == ScreenEventType::UltimateStopped)
            .expect("should emit UltimateStopped");
        let residual = stopped.payload["ultimate_charge"].as_u64().unwrap() as u32;

        assert!(residual <= start_charge, "residual must not exceed start charge");
        assert!(
            residual >= start_charge * 9 / 10,
            "residual {residual} should be ≥90% of start_charge {start_charge} on immediate cancel"
        );
    }

    #[test]
    fn oracle_triggered_payload_contains_activation_charge() {
        let mut engine = oracle_engine();
        let charge_max = engine.character.stats().charge_profile.charge_max;
        let half = charge_max / 2;
        engine.state.ultimate_charge = half;

        let envelopes = engine.process_ulti_press(Instant::now());
        let triggered = envelopes
            .iter()
            .find(|e| e.event_type == ScreenEventType::UltimateTriggered)
            .unwrap();
        assert_eq!(
            triggered.payload["activation_charge"].as_u64().unwrap() as u32,
            half
        );
    }

    // ── Ghost partial activation ───────────────────────────────────────────────

    #[test]
    fn ghost_does_not_advance_cycle_on_rejected_press() {
        let mut engine = ghost_engine();
        // Index 0 = multiball_split — requires full charge.
        engine.state.ghost_cycle_index = 0;
        let charge_max = engine.character.stats().charge_profile.charge_max;
        engine.state.ultimate_charge = charge_max / 2;

        let idx_before = engine.state.ghost_cycle_index;
        engine.process_ulti_press(Instant::now());
        assert_eq!(
            engine.state.ghost_cycle_index, idx_before,
            "cycle must not advance on a rejected press"
        );
    }

    #[test]
    fn ghost_triggers_partial_when_time_slow() {
        let mut engine = ghost_engine();
        // Index 2 = time_slow.
        engine.state.ghost_cycle_index = 2;
        let charge_max = engine.character.stats().charge_profile.charge_max;
        let min = activation_min_charge_for(charge_max, config::get().oracle_activation_min_ratio);
        engine.state.ultimate_charge = min;

        let envelopes = engine.process_ulti_press(Instant::now());
        let triggered = envelopes
            .iter()
            .find(|e| e.event_type == ScreenEventType::UltimateTriggered)
            .expect("should trigger");
        assert_eq!(triggered.payload["ulti_id"], serde_json::json!("time_slow"));
        assert_eq!(engine.state.ghost_cycle_index, 3, "cycle must advance after acceptance");
    }

    #[test]
    fn ghost_requires_full_charge_for_multiball_split() {
        let mut engine = ghost_engine();
        engine.state.ghost_cycle_index = 0;
        let charge_max = engine.character.stats().charge_profile.charge_max;
        engine.state.ultimate_charge = charge_max - 1;

        let envelopes = engine.process_ulti_press(Instant::now());
        assert!(
            !envelopes
                .iter()
                .any(|e| e.event_type == ScreenEventType::UltimateTriggered),
            "multiball_split should require full charge"
        );
    }

    #[test]
    fn ghost_requires_full_charge_for_rampage() {
        let mut engine = ghost_engine();
        engine.state.ghost_cycle_index = 1;
        let charge_max = engine.character.stats().charge_profile.charge_max;
        engine.state.ultimate_charge = charge_max - 1;

        let envelopes = engine.process_ulti_press(Instant::now());
        assert!(
            !envelopes
                .iter()
                .any(|e| e.event_type == ScreenEventType::UltimateTriggered),
            "rampage should require full charge"
        );
    }

    // ── ScoreUpdate.ulti_ready ─────────────────────────────────────────────────

    #[test]
    fn ulti_ready_true_for_oracle_at_threshold() {
        let mut engine = oracle_engine();
        let charge_max = engine.character.stats().charge_profile.charge_max;
        let min = activation_min_charge_for(charge_max, config::get().oracle_activation_min_ratio);
        engine.state.ultimate_charge = min;
        let update = engine.emit_score_update(None);
        assert_eq!(update.payload["ulti_ready"], serde_json::json!(true));
    }

    #[test]
    fn ulti_ready_false_for_oracle_below_threshold() {
        let mut engine = oracle_engine();
        let charge_max = engine.character.stats().charge_profile.charge_max;
        let min = activation_min_charge_for(charge_max, config::get().oracle_activation_min_ratio);
        if min == 0 {
            return;
        }
        engine.state.ultimate_charge = min - 1;
        let update = engine.emit_score_update(None);
        assert_eq!(update.payload["ulti_ready"], serde_json::json!(false));
    }

    #[test]
    fn ulti_ready_true_for_ghost_time_slow_at_threshold() {
        let mut engine = ghost_engine();
        engine.state.ghost_cycle_index = 2; // time_slow slot
        let charge_max = engine.character.stats().charge_profile.charge_max;
        let min = activation_min_charge_for(charge_max, config::get().oracle_activation_min_ratio);
        engine.state.ultimate_charge = min;
        let update = engine.emit_score_update(None);
        assert_eq!(update.payload["ulti_ready"], serde_json::json!(true));
    }

    #[test]
    fn ulti_ready_false_for_ghost_multiball_below_full_charge() {
        let mut engine = ghost_engine();
        engine.state.ghost_cycle_index = 0; // multiball_split slot
        let charge_max = engine.character.stats().charge_profile.charge_max;
        engine.state.ultimate_charge = charge_max - 1;
        let update = engine.emit_score_update(None);
        assert_eq!(update.payload["ulti_ready"], serde_json::json!(false));
    }

    // ── Regression: existing full-charge behaviour ─────────────────────────────

    #[test]
    fn viper_still_requires_full_charge() {
        let mut engine = GameEngine::new("viper");
        engine.process(GameEvent::StartGame);
        let charge_max = engine.character.stats().charge_profile.charge_max;
        engine.state.ultimate_charge = charge_max - 1;

        let envelopes = engine.process_ulti_press(Instant::now());
        assert!(
            !envelopes
                .iter()
                .any(|e| e.event_type == ScreenEventType::UltimateTriggered),
            "viper rampage should still need full charge"
        );
    }

    #[test]
    fn enforcer_still_requires_full_charge() {
        let mut engine = GameEngine::new("enforcer");
        engine.process(GameEvent::StartGame);
        let charge_max = engine.character.stats().charge_profile.charge_max;
        engine.state.ultimate_charge = charge_max - 1;

        let envelopes = engine.process_ulti_press(Instant::now());
        assert!(
            !envelopes
                .iter()
                .any(|e| e.event_type == ScreenEventType::UltimateTriggered),
            "enforcer multiball should still need full charge"
        );
    }
}
