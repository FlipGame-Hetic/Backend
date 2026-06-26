//! Central game engine: receives raw hardware/screen events and produces
//! `ScreenEnvelope`s to broadcast back to the frontend.

use std::time::Instant;

use shared::screen::{ScreenEnvelope, ScreenEventType, ScreenId, ScreenTarget};

use crate::combo::{ComboDetector, MultiplierState, StreakState};
use crate::engine::config;
use crate::engine::pve::PveEngine;
use crate::engine::scoring::timer_bonus;
use crate::engine::states::GameState;
use crate::player::personnages::character::{Character, select_character};

mod charge;
mod emit;
mod input;
mod process;
mod ulti;

/// Ghost cycles through these ulti IDs in order (index mod 3).
pub(super) const GHOST_CYCLE: [&str; 3] = ["multiball_split", "rampage", "time_slow"];

/// Where a scoring event originated — controls the per-character charge weight.
pub(super) enum ChargeSource {
    Bumper,
    Rail,
    Combo,
    Other,
}

/// Top-level game engine owned by the session actor.
/// Holds all subsystems (combo, streak, multiplier, PVE, character) and the authoritative `GameState`.
pub struct GameEngine {
    pub state: GameState,
    combo_detector: ComboDetector,
    multiplier: MultiplierState,
    streak: StreakState,
    pve_engine: PveEngine,
    character: Box<dyn Character>,
    /// Prevents the timer bonus from being awarded more than once per game.
    timer_bonus_given: bool,
}

impl GameEngine {
    pub fn new(character_slug: &str) -> Self {
        Self {
            state: GameState::new(config::get().default_lives),
            combo_detector: ComboDetector::new(),
            multiplier: MultiplierState::new(),
            streak: StreakState::new(),
            pve_engine: PveEngine::new(),
            character: select_character(character_slug),
            timer_bonus_given: false,
        }
    }

    /// Final multiplier applied to all scoring: ulti override takes priority,
    /// otherwise streak × combo multiplier.
    fn effective_multiplier(&self, now: Instant) -> f32 {
        if self.state.is_ulti_active(now)
            && let Some(override_mult) = self.state.ulti_multiplier_override
        {
            return override_mult;
        }
        self.multiplier.current(now) * self.streak.current()
    }

    fn emit_multiplier_update(&self, now: Instant) -> ScreenEnvelope {
        make_event_envelope(
            ScreenEventType::MultiplierUpdate,
            serde_json::json!({
                "multiplier": self.effective_multiplier(now),
                "duration_ms": config::get().streak_window_ms,
            }),
        )
    }

    /// Tick the PVE engine for cooldown/transition progression.
    /// Also advances time-based character charge (Oracle).
    pub fn pve_tick(&mut self, now: Instant) -> Vec<ScreenEnvelope> {
        self.tick_time_charge(now);

        let (envelopes, extra) = self.pve_engine.tick(now);
        let mut all = envelopes;
        for e in extra {
            all.extend(self.process(e));
        }
        all
    }

    /// Build a point-in-time snapshot for the HTTP API response (score + multiplier + boss HP).
    pub fn take_snapshot(&self) -> crate::GameSnapshot {
        let now = Instant::now();
        let max_hp = self.pve_engine.boss_max_hp();
        let boss_hp_percent = if max_hp > 0 {
            Some(self.pve_engine.boss_hp() as f32 / max_hp as f32)
        } else {
            None
        };
        crate::GameSnapshot {
            state: self.state.clone(),
            current_multiplier: self.effective_multiplier(now),
            boss_hp_percent,
        }
    }

    /// Award the timer bonus once when the session reaches `timer_bonus_seconds`
    /// without losing a ball. No-op after the first award.
    fn check_timer_bonus(&mut self, now: Instant) -> Vec<ScreenEnvelope> {
        if self.timer_bonus_given {
            return vec![];
        }
        let Some(start) = self.state.session_start else {
            return vec![];
        };
        let elapsed = now.duration_since(start).as_secs();
        if elapsed >= config::get().timer_bonus_seconds && self.state.balls_lost_since_start == 0 {
            self.timer_bonus_given = true;
            let old_score = self.state.score;
            self.state.score = timer_bonus(self.state.score, 0);
            let delta = self.state.score.saturating_sub(old_score);
            return vec![
                make_event_envelope(
                    ScreenEventType::TimerBonus,
                    serde_json::json!({ "new_score": self.state.score }),
                ),
                self.emit_score_delta(delta, "timer_bonus"),
                self.emit_score_update(None),
            ];
        }
        vec![]
    }
}

pub(super) fn make_event_envelope(
    event_type: ScreenEventType,
    payload: serde_json::Value,
) -> ScreenEnvelope {
    ScreenEnvelope {
        from: ScreenId::GameEngine,
        to: ScreenTarget::Broadcast,
        event_type,
        payload,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::events::{GameEvent, GameOverReason};

    fn started_engine() -> GameEngine {
        let mut engine = GameEngine::new("enforcer");
        engine.process(GameEvent::StartGame);
        engine
    }

    #[test]
    fn rail_tick_increases_score() {
        let mut engine = started_engine();
        let before = engine.state.score;
        let envelopes = engine.process(GameEvent::RailTick {
            ball_id: None,
            fib_step: 0,
        });
        assert!(
            engine.state.score > before,
            "score should increase after RailTick"
        );
        assert!(
            envelopes
                .iter()
                .any(|e| e.event_type == ScreenEventType::ScoreDelta),
            "should emit ScoreDelta"
        );
        assert!(
            envelopes
                .iter()
                .any(|e| e.event_type == ScreenEventType::ScoreUpdate),
            "should emit ScoreUpdate"
        );
    }

    #[test]
    fn rail_tick_fibonacci_progression() {
        let delta_at = |fib_step: u32| {
            let mut engine = started_engine();
            let before = engine.state.score;
            engine.process(GameEvent::RailTick {
                ball_id: None,
                fib_step,
            });
            engine.state.score - before
        };

        let d0 = delta_at(0);
        let d1 = delta_at(1);
        let d2 = delta_at(2);

        assert_eq!(
            d0, d1,
            "fib(0)==fib(1) so step-0 and step-1 deltas should be equal"
        );
        assert!(
            d2 > d1,
            "step-2 delta should be larger than step-1 (fib grows)"
        );
    }

    #[test]
    fn rail_tick_includes_ball_id_in_delta() {
        let mut engine = started_engine();
        let envelopes = engine.process(GameEvent::RailTick {
            ball_id: Some("ball-uuid-2".to_string()),
            fib_step: 0,
        });
        let delta_env = envelopes
            .iter()
            .find(|e| e.event_type == ScreenEventType::ScoreDelta)
            .expect("ScoreDelta should be emitted");
        assert_eq!(
            delta_env.payload["ball_id"],
            serde_json::json!("ball-uuid-2")
        );
    }

    #[test]
    fn rail_tick_no_ball_id_omits_field() {
        let mut engine = started_engine();
        let envelopes = engine.process(GameEvent::RailTick {
            ball_id: None,
            fib_step: 0,
        });
        let delta_env = envelopes
            .iter()
            .find(|e| e.event_type == ScreenEventType::ScoreDelta)
            .expect("ScoreDelta should be emitted");
        assert!(
            delta_env.payload.get("ball_id").is_none(),
            "ball_id should be absent when None"
        );
    }

    #[test]
    fn rail_tick_ignored_when_not_in_game() {
        let mut engine = GameEngine::new("enforcer");
        let before = engine.state.score;
        engine.process(GameEvent::RailTick {
            ball_id: None,
            fib_step: 0,
        });
        assert_eq!(
            engine.state.score, before,
            "tick outside InGame must not change score"
        );
    }

    #[test]
    fn rail_tick_ignored_when_cheating_detected() {
        let mut engine = started_engine();
        engine.state.cheating_detected = true;
        let before = engine.state.score;
        engine.process(GameEvent::RailTick {
            ball_id: None,
            fib_step: 0,
        });
        assert_eq!(
            engine.state.score, before,
            "score must be locked when cheating detected"
        );
    }

    #[test]
    fn multiball_two_balls_score_independently() {
        let delta_for = |ball_id: &str| {
            let mut engine = started_engine();
            let before = engine.state.score;
            engine.process(GameEvent::RailTick {
                ball_id: Some(ball_id.to_string()),
                fib_step: 3,
            });
            engine.state.score - before
        };

        let d1 = delta_for("ball-uuid-1");
        let d2 = delta_for("ball-uuid-2");
        assert!(d1 > 0);
        assert_eq!(d1, d2, "same fib_step → same delta regardless of ball_id");
    }

    #[test]
    fn game_over_ignored_rail_tick() {
        let mut engine = started_engine();
        engine.process(GameEvent::GameOverTriggered {
            reason: GameOverReason::NoLivesLeft,
        });
        let before = engine.state.score;
        engine.process(GameEvent::RailTick {
            ball_id: None,
            fib_step: 0,
        });
        assert_eq!(
            engine.state.score, before,
            "tick after GameOver must not change score"
        );
    }

    #[test]
    fn rail_tick_includes_ball_id_in_score_update() {
        let mut engine = started_engine();
        let envelopes = engine.process(GameEvent::RailTick {
            ball_id: Some("ball-uuid-2".to_string()),
            fib_step: 0,
        });
        let update_env = envelopes
            .iter()
            .find(|e| e.event_type == ScreenEventType::ScoreUpdate)
            .expect("ScoreUpdate should be emitted");
        assert_eq!(
            update_env.payload["ball_id"],
            serde_json::json!("ball-uuid-2")
        );
    }

    #[test]
    fn bumper_hit_includes_ball_id_in_score_events() {
        let mut engine = started_engine();
        let envelopes = engine.process(GameEvent::BumperHit {
            pts: 100,
            ball_id: Some("ball-abc".to_string()),
        });
        let delta_env = envelopes
            .iter()
            .find(|e| e.event_type == ScreenEventType::ScoreDelta)
            .expect("ScoreDelta should be emitted");
        assert_eq!(delta_env.payload["ball_id"], serde_json::json!("ball-abc"));

        let update_env = envelopes
            .iter()
            .find(|e| e.event_type == ScreenEventType::ScoreUpdate)
            .expect("ScoreUpdate should be emitted");
        assert_eq!(update_env.payload["ball_id"], serde_json::json!("ball-abc"));
    }

    #[test]
    fn bumper_hit_no_ball_id_omits_field() {
        let mut engine = started_engine();
        let envelopes = engine.process(GameEvent::BumperHit {
            pts: 100,
            ball_id: None,
        });
        let delta_env = envelopes
            .iter()
            .find(|e| e.event_type == ScreenEventType::ScoreDelta)
            .expect("ScoreDelta should be emitted");
        assert!(delta_env.payload.get("ball_id").is_none());

        let update_env = envelopes
            .iter()
            .find(|e| e.event_type == ScreenEventType::ScoreUpdate)
            .expect("ScoreUpdate should be emitted");
        assert!(update_env.payload.get("ball_id").is_none());
    }

    #[test]
    fn score_update_includes_charge_fields() {
        let mut engine = started_engine();
        let envelopes = engine.process(GameEvent::BumperHit {
            pts: 100,
            ball_id: None,
        });
        let update = envelopes
            .iter()
            .find(|e| e.event_type == ScreenEventType::ScoreUpdate)
            .expect("ScoreUpdate should be emitted");
        assert!(update.payload.get("ultimate_charge").is_some());
        assert!(update.payload.get("ultimate_max").is_some());
        assert!(update.payload.get("ulti_ready").is_some());
    }

    #[test]
    fn bumper_charge_accumulates_with_buffer() {
        let mut engine = started_engine();
        // Enforcer bumper weight = 1.0; ultime_charge_ratio = 100.
        // 1 bumper hit = 100 pts → 100 / 100 = 1 charge unit.
        for _ in 0..5 {
            engine.process(GameEvent::BumperHit {
                pts: 100,
                ball_id: None,
            });
        }
        assert!(
            engine.state.ultimate_charge >= 5,
            "charge should accumulate"
        );
    }

    #[test]
    fn viper_ulti_triggers_when_full() {
        let mut engine = GameEngine::new("viper");
        engine.process(GameEvent::StartGame);
        let charge_max = engine.character.stats().charge_profile.charge_max;
        engine.state.ultimate_charge = charge_max;

        let now = Instant::now();
        let envelopes = engine.process_ulti_press(now);
        assert!(
            envelopes
                .iter()
                .any(|e| e.event_type == ScreenEventType::UltimateTriggered),
            "should emit UltimateTriggered"
        );
        assert!(
            engine.state.is_ulti_active(Instant::now()),
            "ulti should be active"
        );
        assert_eq!(
            engine.state.ulti_multiplier_override,
            Some(config::get().viper_rampage_multiplier)
        );
    }

    #[test]
    fn oracle_ulti_is_cancellable() {
        let mut engine = GameEngine::new("oracle");
        engine.process(GameEvent::StartGame);
        let charge_max = engine.character.stats().charge_profile.charge_max;
        engine.state.ultimate_charge = charge_max;

        let now = Instant::now();
        engine.process_ulti_press(now);
        assert!(engine.state.is_ulti_active(Instant::now()));

        let now2 = Instant::now();
        let envelopes = engine.process_ulti_press(now2);
        assert!(
            envelopes
                .iter()
                .any(|e| e.event_type == ScreenEventType::UltimateStopped),
            "should emit UltimateStopped on cancel"
        );
        assert!(
            !engine.state.is_ulti_active(Instant::now()),
            "ulti should be cancelled"
        );
    }

    #[test]
    fn ghost_cycle_advances_on_each_activation() {
        let mut engine = GameEngine::new("ghost");
        engine.process(GameEvent::StartGame);
        let charge_max = engine.character.stats().charge_profile.charge_max;

        for expected_ulti in &["multiball_split", "rampage", "time_slow"] {
            engine.state.ultimate_charge = charge_max;
            let now = Instant::now();
            let envelopes = engine.process_ulti_press(now);
            let triggered = envelopes
                .iter()
                .find(|e| e.event_type == ScreenEventType::UltimateTriggered)
                .expect("should emit UltimateTriggered");
            assert_eq!(
                triggered.payload["ulti_id"],
                serde_json::json!(expected_ulti)
            );

            engine.state.ulti_ends_at = Some(Instant::now());
            engine.state.ulti_active_id = None;
            engine.state.ulti_multiplier_override = None;
        }
    }
}
