use std::time::Instant;

use shared::screen::{ScreenEnvelope, ScreenEventType};

use crate::combo::{ComboDetector, ComboResult, MultiplierState};
use crate::engine::config;
use crate::engine::events::{GameEvent, GameOverReason};
use crate::engine::pve::PveEngine;
use crate::engine::scoring::{
    apply_tilt_penalty, rail_tick_score, score_bumper, score_bumper_triangle, score_portal_bonus,
};
use crate::engine::states::{GamePhase, GameState, TiltEffect};

use super::{ChargeSource, GameEngine, make_event_envelope};

impl GameEngine {
    pub fn process(&mut self, event: GameEvent) -> Vec<ScreenEnvelope> {
        let now = Instant::now();
        let mut envelopes = Vec::new();

        // Lazily expire any sustained ulti that has run its full duration.
        self.try_expire_ulti(now);

        match event {
            GameEvent::StartGame => {
                self.state = GameState::new(config::get().default_lives);
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
                self.add_charge(pts as u64, ChargeSource::Bumper, now);

                let (pve_env, extra) = self.pve_engine.on_score_delta(scored);
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
                let pts = score_portal_bonus();
                self.state.add_score(pts);
                self.add_charge(config::get().portal_score as u64, ChargeSource::Other, now);
                let (pve_env, extra) = self.pve_engine.on_score_delta(pts);
                envelopes.extend(pve_env);
                for e in extra {
                    envelopes.extend(self.process(e));
                }
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
                let pts = config::get().ball_saver_score as u64;
                self.state.add_score(pts);
                self.add_charge(pts, ChargeSource::Other, now);
                let (pve_env, extra) = self.pve_engine.on_score_delta(pts);
                envelopes.extend(pve_env);
                for e in extra {
                    envelopes.extend(self.process(e));
                }
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
                let pts = config::get().multiball_score as u64;
                self.state.add_score(pts);
                self.add_charge(pts, ChargeSource::Other, now);
                let (pve_env, extra) = self.pve_engine.on_score_delta(pts);
                envelopes.extend(pve_env);
                for e in extra {
                    envelopes.extend(self.process(e));
                }
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

            // Kept in GameEvent for backward compatibility but no longer the activation path.
            GameEvent::UltimateActivated { .. } => {}

            GameEvent::ComboActivated(effect) => {
                let (_streak_changed, streak_armed) = self.streak.record(now);
                let current_multiplier = self.effective_multiplier(now);
                let scaled_bonus = (effect.bonus_pts as f32 * current_multiplier) as u64;
                self.state.add_score(scaled_bonus);
                if effect.bonus_pts > 0 {
                    self.add_charge(effect.bonus_pts as u64, ChargeSource::Combo, now);
                    let (pve_env, extra) = self.pve_engine.on_score_delta(scaled_bonus);
                    envelopes.extend(pve_env);
                    for e in extra {
                        envelopes.extend(self.process(e));
                    }
                }
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
                let current_multiplier = self.multiplier.current(now);
                let scored = rail_tick_score(fib_step, current_multiplier);
                self.state.add_score(scored);
                let base_pts = rail_tick_score(fib_step, 1.0);
                self.add_charge(base_pts, ChargeSource::Rail, now);
                let (pve_env, extra) = self.pve_engine.on_score_delta(scored);
                envelopes.extend(pve_env);
                for e in extra {
                    envelopes.extend(self.process(e));
                }
                envelopes.push(self.emit_scored_delta(scored, "rail", ball_id));
                envelopes.push(self.emit_score_update(bid));
            }
        }

        envelopes
    }
}
