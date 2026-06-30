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
                self.ball_in_play = false;
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
                // Combo sequence only advances when the ball is physically on the playfield.
                // Flipper actions still pass through; only the combo counter is gated.
                if !self.ball_in_play {
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

#[cfg(test)]
mod tests {
    use shared::screen::ScreenEventType;

    use crate::engine::events::{GameEvent, GameOverReason};
    use crate::engine::states::GamePhase;

    use super::GameEngine;

    fn started(slug: &str) -> GameEngine {
        let mut e = GameEngine::new(slug);
        e.process(GameEvent::StartGame);
        e
    }

    // EndGame

    #[test]
    fn end_game_sets_phase_game_over() {
        let mut engine = started("enforcer");
        engine.process(GameEvent::EndGame);
        assert_eq!(engine.state.phase, GamePhase::GameOver);
    }

    #[test]
    fn end_game_emits_game_over_event() {
        let mut engine = started("enforcer");
        let evs = engine.process(GameEvent::EndGame);
        assert!(
            evs.iter()
                .any(|e| e.event_type == ScreenEventType::GameOver)
        );
    }

    // BallLaunched / BallSaved

    #[test]
    fn ball_launched_is_no_op() {
        let mut engine = started("enforcer");
        let before = engine.state.score;
        let evs = engine.process(GameEvent::BallLaunched);
        assert!(evs.is_empty());
        assert_eq!(engine.state.score, before);
    }

    #[test]
    fn ball_saved_is_no_op() {
        let mut engine = started("enforcer");
        let evs = engine.process(GameEvent::BallSaved);
        assert!(evs.is_empty());
    }

    // BallLost

    #[test]
    fn ball_lost_decrements_lives() {
        let mut engine = started("enforcer");
        let before = engine.state.lives;
        engine.process(GameEvent::BallLost);
        assert_eq!(engine.state.lives, before - 1);
    }

    #[test]
    fn ball_lost_emits_life_update_when_lives_remain() {
        let mut engine = started("enforcer");
        // default 3 lives → losing one leaves 2
        let evs = engine.process(GameEvent::BallLost);
        assert!(
            evs.iter()
                .any(|e| e.event_type == ScreenEventType::LifeUpdate)
        );
    }

    #[test]
    fn ball_lost_increments_balls_lost_counter() {
        let mut engine = started("enforcer");
        engine.process(GameEvent::BallLost);
        assert_eq!(engine.state.balls_lost_since_start, 1);
    }

    #[test]
    fn ball_lost_on_last_life_triggers_game_over() {
        let mut engine = started("enforcer");
        engine.state.lives = 1;
        let evs = engine.process(GameEvent::BallLost);
        assert_eq!(engine.state.phase, GamePhase::GameOver);
        assert!(
            evs.iter()
                .any(|e| e.event_type == ScreenEventType::GameOver)
        );
    }

    #[test]
    fn ball_lost_ignored_outside_in_game() {
        let mut engine = GameEngine::new("enforcer");
        // still Idle, not started
        let evs = engine.process(GameEvent::BallLost);
        assert!(evs.is_empty());
        assert_eq!(engine.state.balls_lost_since_start, 0);
    }

    // TiltDetected

    #[test]
    fn first_tilt_emits_penalty() {
        let mut engine = started("enforcer");
        engine.state.score = 10_000;
        let evs = engine.process(GameEvent::TiltDetected);
        assert!(
            evs.iter()
                .any(|e| e.event_type == ScreenEventType::TiltPenalty)
        );
        assert!(engine.state.score < 10_000);
    }

    #[test]
    fn third_tilt_locks_score() {
        let mut engine = started("enforcer");
        engine.process(GameEvent::TiltDetected);
        engine.process(GameEvent::TiltDetected);
        engine.process(GameEvent::TiltDetected);
        assert!(engine.state.cheating_detected);
        let evs = engine.state.tilt_state.count;
        assert_eq!(evs, 3);
    }

    #[test]
    fn third_tilt_emits_cheating_detected() {
        let mut engine = started("enforcer");
        engine.process(GameEvent::TiltDetected);
        engine.process(GameEvent::TiltDetected);
        let evs = engine.process(GameEvent::TiltDetected);
        assert!(
            evs.iter()
                .any(|e| e.event_type == ScreenEventType::CheatingDetected)
        );
    }

    // LifeUp

    #[test]
    fn life_up_increments_lives() {
        let mut engine = started("enforcer");
        let before = engine.state.lives;
        engine.process(GameEvent::LifeUp);
        assert_eq!(engine.state.lives, before + 1);
    }

    #[test]
    fn life_up_emits_life_update() {
        let mut engine = started("enforcer");
        let evs = engine.process(GameEvent::LifeUp);
        assert!(
            evs.iter()
                .any(|e| e.event_type == ScreenEventType::LifeUpdate)
        );
    }

    // MultiballTriggered / MultiballWin

    #[test]
    fn multiball_triggered_scores_and_emits_multiball_win() {
        let mut engine = started("enforcer");
        let before = engine.state.score;
        let evs = engine.process(GameEvent::MultiballTriggered);
        assert!(engine.state.score > before);
        assert!(
            evs.iter()
                .any(|e| e.event_type == ScreenEventType::MultiballWin)
        );
    }

    #[test]
    fn multiball_triggered_ignored_outside_in_game() {
        let mut engine = GameEngine::new("enforcer");
        let evs = engine.process(GameEvent::MultiballTriggered);
        assert!(evs.is_empty());
    }

    #[test]
    fn multiball_win_emits_score_delta_and_update() {
        let mut engine = started("enforcer");
        let evs = engine.process(GameEvent::MultiballWin);
        assert!(
            evs.iter()
                .any(|e| e.event_type == ScreenEventType::ScoreDelta)
        );
        assert!(
            evs.iter()
                .any(|e| e.event_type == ScreenEventType::ScoreUpdate)
        );
    }

    // PortalUsed

    #[test]
    fn portal_used_adds_score_and_emits_events() {
        let mut engine = started("enforcer");
        let before = engine.state.score;
        let evs = engine.process(GameEvent::PortalUsed { ball_id: None });
        assert!(engine.state.score > before);
        assert!(
            evs.iter()
                .any(|e| e.event_type == ScreenEventType::ScoreDelta)
        );
        assert!(
            evs.iter()
                .any(|e| e.event_type == ScreenEventType::ScoreUpdate)
        );
    }

    #[test]
    fn portal_used_ball_id_propagated() {
        let mut engine = started("enforcer");
        let evs = engine.process(GameEvent::PortalUsed {
            ball_id: Some("ball-xyz".into()),
        });
        let update = evs
            .iter()
            .find(|e| e.event_type == ScreenEventType::ScoreUpdate)
            .unwrap();
        assert_eq!(update.payload["ball_id"], serde_json::json!("ball-xyz"));
    }

    // BallSaverReady

    #[test]
    fn ball_saver_ready_adds_score() {
        let mut engine = started("enforcer");
        let before = engine.state.score;
        engine.process(GameEvent::BallSaverReady);
        assert!(engine.state.score > before);
    }

    #[test]
    fn ball_saver_ready_emits_ball_saver_event() {
        let mut engine = started("enforcer");
        let evs = engine.process(GameEvent::BallSaverReady);
        assert!(
            evs.iter()
                .any(|e| e.event_type == ScreenEventType::BallSaverReady)
        );
    }

    #[test]
    fn ball_saver_ready_ignored_outside_in_game() {
        let mut engine = GameEngine::new("enforcer");
        let evs = engine.process(GameEvent::BallSaverReady);
        assert!(evs.is_empty());
    }

    // BumperTriangleHit

    #[test]
    fn bumper_triangle_scores_more_than_regular_bumper() {
        let score_for = |event: GameEvent| {
            let mut e = started("enforcer");
            let before = e.state.score;
            e.process(event);
            e.state.score - before
        };
        let regular = score_for(GameEvent::BumperHit {
            pts: 100,
            ball_id: None,
        });
        let triangle = score_for(GameEvent::BumperTriangleHit {
            pts: 150,
            ball_id: None,
        });
        assert!(triangle > regular);
    }

    // BossDefeated

    #[test]
    fn boss_defeated_emits_event_with_id() {
        let mut engine = started("enforcer");
        let evs = engine.process(GameEvent::BossDefeated { boss_id: 2 });
        let ev = evs
            .iter()
            .find(|e| e.event_type == ScreenEventType::BossDefeated)
            .expect("should emit BossDefeated");
        assert_eq!(ev.payload["boss_id"], serde_json::json!(2));
    }

    // GameOverTriggered

    #[test]
    fn game_over_triggered_sets_phase() {
        let mut engine = started("enforcer");
        engine.process(GameEvent::GameOverTriggered {
            reason: GameOverReason::PlayerQuit,
        });
        assert_eq!(engine.state.phase, GamePhase::GameOver);
    }

    // ScoreMultiplierActivated

    #[test]
    fn score_multiplier_activated_emits_multiplier_update() {
        let mut engine = started("enforcer");
        let evs = engine.process(GameEvent::ScoreMultiplierActivated);
        assert!(
            evs.iter()
                .any(|e| e.event_type == ScreenEventType::MultiplierUpdate)
        );
    }

    // UltimateActivated (legacy no-op)

    #[test]
    fn ultimate_activated_legacy_is_no_op() {
        let mut engine = started("enforcer");
        let before = engine.state.score;
        let evs = engine.process(GameEvent::UltimateActivated {
            player_id: "p1".into(),
        });
        assert!(evs.is_empty());
        assert_eq!(engine.state.score, before);
    }

    // ComboActivated

    #[test]
    fn combo_activated_with_bonus_adds_score() {
        let mut engine = started("enforcer");
        let before = engine.state.score;
        engine.process(GameEvent::ComboActivated(crate::combo::ComboEffect {
            combo_id: 0,
            bonus_pts: 1_000,
            sequence: vec![],
        }));
        assert!(engine.state.score > before);
    }

    #[test]
    fn combo_activated_emits_combo_activated_event() {
        let mut engine = started("enforcer");
        let evs = engine.process(GameEvent::ComboActivated(crate::combo::ComboEffect {
            combo_id: 0,
            bonus_pts: 500,
            sequence: vec![],
        }));
        assert!(
            evs.iter()
                .any(|e| e.event_type == ScreenEventType::ComboActivated)
        );
    }

    #[test]
    fn combo_activated_zero_bonus_does_not_add_score() {
        let mut engine = started("enforcer");
        let before = engine.state.score;
        engine.process(GameEvent::ComboActivated(crate::combo::ComboEffect {
            combo_id: 0,
            bonus_pts: 0,
            sequence: vec![],
        }));
        assert_eq!(engine.state.score, before);
    }

    // ButtonPressed outside InGame

    #[test]
    fn button_pressed_ignored_outside_in_game() {
        let mut engine = GameEngine::new("enforcer");
        let evs = engine.process(GameEvent::ButtonPressed {
            side: crate::engine::events::ButtonSide::Left,
        });
        assert!(evs.is_empty());
    }
}
