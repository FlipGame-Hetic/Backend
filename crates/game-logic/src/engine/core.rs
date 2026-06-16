use std::time::Instant;

use shared::events::InboundMessage;
use shared::screen::{ScreenEnvelope, ScreenEventType, ScreenId, ScreenTarget};

use crate::combo::{ComboDetector, ComboResult, MultiplierState, StreakState};
use crate::engine::config::{DEFAULT_LIVES, ULTIME_CHARGE_RATIO};
use crate::engine::events::{ButtonSide, GameEvent, GameOverReason};
use crate::engine::pve::PveEngine;
use crate::engine::scoring::{
    apply_tilt_penalty, rail_tick_score, score_bumper, score_bumper_triangle, timer_bonus,
};
use crate::engine::states::{GamePhase, GameState, TiltEffect};
use crate::player::personnages::character::{Character, select_character};
use crate::player::skills::player_bonus::SkillEffect;

pub struct GameEngine {
    pub state: GameState,
    combo_detector: ComboDetector,
    multiplier: MultiplierState,
    streak: StreakState,
    pve_engine: PveEngine,
    character: Box<dyn Character>,
    timer_bonus_given: bool,
}

impl GameEngine {
    pub fn new(character_id: u8) -> Self {
        Self {
            state: GameState::new(DEFAULT_LIVES),
            combo_detector: ComboDetector::new(),
            multiplier: MultiplierState::new(),
            streak: StreakState::new(),
            pve_engine: PveEngine::new(),
            character: select_character(character_id),
            timer_bonus_given: false,
        }
    }

    fn effective_multiplier(&self, now: Instant) -> f32 {
        self.multiplier.current(now) * self.streak.current()
    }

    fn emit_multiplier_update(&self, now: Instant) -> ScreenEnvelope {
        make_event_envelope(
            ScreenEventType::MultiplierUpdate,
            serde_json::json!({
                "multiplier": self.effective_multiplier(now),
                "duration_ms": crate::engine::config::STREAK_WINDOW_MS,
            }),
        )
    }

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

    pub fn handle_inbound(&mut self, msg: &InboundMessage) -> Vec<ScreenEnvelope> {
        match msg {
            InboundMessage::Button(btn) if btn.state > 0 => {
                if let Some(side) = ButtonSide::from_button_id(&btn.id) {
                    return self.process(GameEvent::ButtonPressed { side });
                }
                vec![]
            }
            InboundMessage::Gyro(gyro) if gyro.tilt => self.process(GameEvent::TiltDetected),
            InboundMessage::Plunger(plunger) if plunger.released => {
                self.process(GameEvent::BallLaunched)
            }
            _ => vec![],
        }
    }

    pub fn handle_screen_event(&mut self, envelope: &ScreenEnvelope) -> Vec<ScreenEnvelope> {
        let event = match &envelope.event_type {
            ScreenEventType::StartGame => {
                let player_id = envelope
                    .payload
                    .get("player_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_owned();
                GameEvent::StartGame { player_id }
            }
            ScreenEventType::EndGame => GameEvent::EndGame,
            ScreenEventType::BallLost => GameEvent::BallLost,
            ScreenEventType::BallSaved => GameEvent::BallSaved,
            ScreenEventType::LifeUp => GameEvent::LifeUp,
            ScreenEventType::UltimateActivated => {
                let player_id = envelope
                    .payload
                    .get("player_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_owned();
                GameEvent::UltimateActivated { player_id }
            }
            ScreenEventType::Bumper => {
                let ball_id = envelope
                    .payload
                    .get("ball_id")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                GameEvent::BumperHit {
                    pts: crate::engine::config::BUMPER_SCORE,
                    ball_id,
                }
            }
            ScreenEventType::BumperTriangle => {
                let ball_id = envelope
                    .payload
                    .get("ball_id")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                GameEvent::BumperTriangleHit {
                    pts: crate::engine::config::BUMPER_TRIANGLE_SCORE,
                    ball_id,
                }
            }
            ScreenEventType::PortalUsed => {
                let ball_id = envelope
                    .payload
                    .get("ball_id")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                GameEvent::PortalUsed { ball_id }
            }
            ScreenEventType::FlipperLeft => GameEvent::ButtonPressed {
                side: ButtonSide::Left,
            },
            ScreenEventType::FlipperRight => GameEvent::ButtonPressed {
                side: ButtonSide::Right,
            },
            ScreenEventType::BallSaverReady => GameEvent::BallSaverReady,
            ScreenEventType::MultiballTriggered => GameEvent::MultiballTriggered,
            other => {
                tracing::debug!(event_type = %other, "unhandled screen event type");
                return vec![];
            }
        };
        self.process(event)
    }

    pub fn process(&mut self, event: GameEvent) -> Vec<ScreenEnvelope> {
        let now = Instant::now();
        let mut envelopes = Vec::new();

        match event {
            GameEvent::StartGame { ref player_id } => {
                self.state = GameState::new(DEFAULT_LIVES);
                self.state.player_id = player_id.clone();
                self.state.phase = GamePhase::InGame;
                self.state.session_start = Some(now);
                self.timer_bonus_given = false;
                self.combo_detector = ComboDetector::new();
                self.multiplier = MultiplierState::new();
                self.streak.reset();
                self.pve_engine = PveEngine::new();

                let (pve_env, extra) = self.pve_engine.on_event(&event, &mut self.state);
                envelopes.extend(pve_env);
                for e in extra {
                    envelopes.extend(self.process(e));
                }
                envelopes.push(self.emit_score_update(None));
                envelopes.push(self.emit_life_update());
            }

            GameEvent::EndGame => {
                self.state.phase = GamePhase::GameOver;
                envelopes.push(self.emit_game_over());
            }

            GameEvent::BallLaunched => {
                tracing::debug!("ball launched");
            }

            GameEvent::BallLost => {
                if self.state.phase != GamePhase::InGame {
                    return envelopes;
                }
                self.state.balls_lost_since_start += 1;
                self.state.lives = self.state.lives.saturating_sub(1);
                self.streak.reset();

                let (pve_env, extra) = self.pve_engine.on_event(&event, &mut self.state);
                envelopes.extend(pve_env);
                for e in extra {
                    envelopes.extend(self.process(e));
                }

                if self.state.lives == 0 {
                    envelopes.extend(self.process(GameEvent::GameOverTriggered {
                        reason: GameOverReason::NoLivesLeft,
                    }));
                } else {
                    envelopes.push(self.emit_life_update());
                }
            }

            GameEvent::BallSaved => {
                tracing::debug!("ball saved");
            }

            GameEvent::ButtonPressed { side } => {
                if self.state.phase != GamePhase::InGame {
                    return envelopes;
                }
                let press = side.into();
                let result = self.combo_detector.push(press, now);

                match result {
                    ComboResult::Activated(effect) => {
                        envelopes.extend(self.process(GameEvent::ComboActivated(effect)));
                    }
                    ComboResult::Penalty { pts } => {
                        if pts < 0 {
                            self.state.score = apply_tilt_penalty(self.state.score, pts);
                        } else {
                            self.state.score = self.state.score.saturating_add(pts as u64);
                        }
                        envelopes.push(self.emit_score_update(None));
                    }
                    ComboResult::BadgeUnlocked { badge_id } => {
                        envelopes.push(make_event_envelope(
                            ScreenEventType::BadgeUnlocked,
                            serde_json::json!({ "badge_id": badge_id }),
                        ));
                    }
                    ComboResult::None => {}
                }
            }

            GameEvent::BumperHit { pts, ref ball_id }
            | GameEvent::BumperTriangleHit { pts, ref ball_id } => {
                if self.state.phase != GamePhase::InGame {
                    return envelopes;
                }
                let bid = ball_id.clone();
                let (_streak_changed, streak_armed) = self.streak.record(now);
                let current_multiplier = self.effective_multiplier(now);
                let scored = match &event {
                    GameEvent::BumperHit { .. } => score_bumper(current_multiplier),
                    _ => score_bumper_triangle(current_multiplier),
                };
                self.state.add_score(scored);
                self.state.ultimate_charge = self
                    .state
                    .ultimate_charge
                    .saturating_add(pts / ULTIME_CHARGE_RATIO);

                let (pve_env, extra) = self.pve_engine.on_event(&event, &mut self.state);
                envelopes.extend(pve_env);
                for e in extra {
                    envelopes.extend(self.process(e));
                }

                if streak_armed {
                    envelopes.push(self.emit_multiplier_update(now));
                }
                envelopes.extend(self.check_timer_bonus(now));
                envelopes.push(self.emit_scored_delta(scored, "bumper", bid.clone()));
                envelopes.push(self.emit_score_update(bid));
            }

            GameEvent::MultiballTriggered => {
                if self.state.phase != GamePhase::InGame {
                    return envelopes;
                }
                envelopes.extend(self.process(GameEvent::MultiballWin));
            }

            GameEvent::PortalUsed { ref ball_id } => {
                let bid = ball_id.clone();
                let (_streak_changed, streak_armed) = self.streak.record(now);
                let pts = crate::engine::scoring::score_portal_bonus();
                self.state.add_score(pts);
                if streak_armed {
                    envelopes.push(self.emit_multiplier_update(now));
                }
                envelopes.push(self.emit_scored_delta(pts, "portal", bid.clone()));
                envelopes.push(self.emit_score_update(bid));
            }

            GameEvent::BallSaverReady => {
                if self.state.phase != GamePhase::InGame {
                    return envelopes;
                }
                let (_streak_changed, streak_armed) = self.streak.record(now);
                let pts = crate::engine::config::BALL_SAVER_SCORE as u64;
                self.state.add_score(pts);
                if streak_armed {
                    envelopes.push(self.emit_multiplier_update(now));
                }
                envelopes.push(self.emit_score_delta(pts, "ball_saver"));
                envelopes.push(make_event_envelope(
                    ScreenEventType::BallSaverReady,
                    serde_json::Value::Null,
                ));
                envelopes.push(self.emit_score_update(None));
            }

            GameEvent::TiltDetected => {
                let effect = self.state.tilt_state.on_tilt();
                match effect {
                    TiltEffect::Penalty(pts) => {
                        self.state.score = apply_tilt_penalty(self.state.score, pts);
                        envelopes.push(self.emit_score_update(None));
                        envelopes.push(make_event_envelope(
                            ScreenEventType::TiltPenalty,
                            serde_json::json!({ "penalty": pts }),
                        ));
                    }
                    TiltEffect::CheatingDetected => {
                        self.state.cheating_detected = true;
                        tracing::warn!("cheating detected — score locked");
                        envelopes.push(make_event_envelope(
                            ScreenEventType::CheatingDetected,
                            serde_json::Value::Null,
                        ));
                    }
                }
            }

            GameEvent::LifeUp => {
                self.state.lives += 1;
                envelopes.push(self.emit_life_update());
            }

            GameEvent::MultiballWin => {
                let (_streak_changed, streak_armed) = self.streak.record(now);
                let pts = crate::engine::config::MULTIBALL_SCORE as u64;
                self.state.add_score(pts);
                if streak_armed {
                    envelopes.push(self.emit_multiplier_update(now));
                }
                envelopes.push(self.emit_score_delta(pts, "multiball"));
                envelopes.push(make_event_envelope(
                    ScreenEventType::MultiballWin,
                    serde_json::Value::Null,
                ));
                envelopes.push(self.emit_score_update(None));
            }

            GameEvent::ScoreMultiplierActivated => {
                envelopes.push(self.emit_multiplier_update(now));
            }

            GameEvent::UltimateActivated { .. } => {
                let charge_max = self.character.stats().ultimate_charge_max;
                if self.state.ultimate_charge >= charge_max {
                    let effect = self.character.bonus().activate(&mut self.state);
                    self.state.ultimate_charge = 0;
                    envelopes.extend(self.apply_skill_effect(effect));
                }
            }

            GameEvent::ComboActivated(effect) => {
                let (_streak_changed, streak_armed) = self.streak.record(now);
                let current_multiplier = self.effective_multiplier(now);
                let scaled_bonus = (effect.bonus_pts as f32 * current_multiplier) as u64;
                self.state.add_score(scaled_bonus);
                envelopes.push(self.emit_combo_activated(&effect));
                if streak_armed {
                    envelopes.push(self.emit_multiplier_update(now));
                }
                envelopes.push(self.emit_score_delta(scaled_bonus, "combo"));
                envelopes.push(self.emit_score_update(None));
            }

            GameEvent::BossDefeated { boss_id } => {
                tracing::info!(boss_id, "boss defeated event processed");
                envelopes.push(make_event_envelope(
                    ScreenEventType::BossDefeated,
                    serde_json::json!({ "boss_id": boss_id }),
                ));
            }

            GameEvent::GameOverTriggered { reason } => {
                self.state.phase = GamePhase::GameOver;
                tracing::info!(?reason, "game over triggered");
                envelopes.push(self.emit_game_over());
            }

            GameEvent::TimerBonusCheck => {
                envelopes.extend(self.check_timer_bonus(now));
            }

            GameEvent::RailTick { ball_id, fib_step } => {
                if self.state.phase != GamePhase::InGame {
                    return envelopes;
                }
                let bid = ball_id.clone();
                // Rail ticks use only the active combo multiplier, not the streak.
                // Counting each tick as a streak hit would drive the streak tier up
                // artificially fast and produce exploding scores.
                let current_multiplier = self.multiplier.current(now);
                let scored = rail_tick_score(fib_step, current_multiplier);
                self.state.add_score(scored);
                envelopes.push(self.emit_scored_delta(scored, "rail", ball_id));
                envelopes.push(self.emit_score_update(bid));
            }
        }

        envelopes
    }

    fn check_timer_bonus(&mut self, now: Instant) -> Vec<ScreenEnvelope> {
        if self.timer_bonus_given {
            return vec![];
        }
        let Some(start) = self.state.session_start else {
            return vec![];
        };
        let elapsed = now.duration_since(start).as_secs();
        if elapsed >= crate::engine::config::TIMER_BONUS_SECONDS
            && self.state.balls_lost_since_start == 0
        {
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

    fn apply_skill_effect(&mut self, effect: SkillEffect) -> Vec<ScreenEnvelope> {
        match effect {
            SkillEffect::ModifyMultiplier {
                factor,
                duration_ms,
            } => {
                let now = Instant::now();
                let combo_effect = crate::combo::ComboEffect {
                    combo_id: 0,
                    bonus_pts: 0,
                    multiplier: factor,
                    duration_ms,
                };
                self.multiplier.apply(&combo_effect, now);
                vec![make_event_envelope(
                    ScreenEventType::MultiplierUpdate,
                    serde_json::json!({ "multiplier": self.effective_multiplier(now), "duration_ms": duration_ms }),
                )]
            }
            SkillEffect::AddBalls { count } => vec![make_event_envelope(
                ScreenEventType::ExtraBall,
                serde_json::json!({ "count": count }),
            )],
            SkillEffect::ShieldActivated { duration_ms } => vec![make_event_envelope(
                ScreenEventType::ShieldActivated,
                serde_json::json!({ "duration_ms": duration_ms }),
            )],
            SkillEffect::AddScore { pts } => {
                self.state.add_score(pts as u64);
                vec![self.emit_score_update(None)]
            }
            SkillEffect::EmitScreenEvent {
                event_type,
                payload,
            } => {
                vec![make_event_envelope(event_type, payload)]
            }
            SkillEffect::NoEffect => vec![],
        }
    }

    fn emit_score_update(&self, ball_id: Option<String>) -> ScreenEnvelope {
        let now = Instant::now();
        let current_multiplier = self.effective_multiplier(now);
        let ball = self.state.balls_lost_since_start + 1;
        let mut payload = serde_json::json!({
            "score": self.state.score,
            "multiplier": current_multiplier,
            "player": self.state.player_id,
            "ball": ball,
        });
        if let Some(bid) = ball_id {
            payload["ball_id"] = serde_json::json!(bid);
        }
        make_event_envelope(ScreenEventType::ScoreUpdate, payload)
    }

    fn emit_score_delta(&self, delta: u64, reason: &str) -> ScreenEnvelope {
        self.emit_scored_delta(delta, reason, None)
    }

    fn emit_scored_delta(
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

    fn emit_life_update(&self) -> ScreenEnvelope {
        make_event_envelope(
            ScreenEventType::LifeUpdate,
            serde_json::json!({ "lives_remaining": self.state.lives }),
        )
    }

    fn emit_combo_activated(&self, effect: &crate::combo::ComboEffect) -> ScreenEnvelope {
        make_event_envelope(
            ScreenEventType::ComboActivated,
            serde_json::json!({
                "combo_id": effect.combo_id,
                "bonus_pts": effect.bonus_pts,
                "multiplier": effect.multiplier,
                "duration_ms": effect.duration_ms,
            }),
        )
    }

    fn emit_game_over(&self) -> ScreenEnvelope {
        make_event_envelope(
            ScreenEventType::GameOver,
            serde_json::json!({ "final_score": self.state.score }),
        )
    }
}

fn make_event_envelope(event_type: ScreenEventType, payload: serde_json::Value) -> ScreenEnvelope {
    ScreenEnvelope {
        from: ScreenId::BackScreen,
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
        let mut engine = GameEngine::new(1);
        engine.process(GameEvent::StartGame {
            player_id: "test".into(),
        });
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
        let mut engine = GameEngine::new(1);
        // Phase is Idle, no StartGame called.
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
}
