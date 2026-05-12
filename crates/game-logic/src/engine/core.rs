use std::time::Instant;

use shared::events::InboundMessage;
use shared::screen::{ScreenEnvelope, ScreenId, ScreenTarget};

use crate::combo::{ComboDetector, ComboResult, MultiplierState};
use crate::engine::config::{DEFAULT_LIVES, ULTIME_CHARGE_RATIO};
use crate::engine::events::{ButtonSide, GameEvent, GameOverReason};
use crate::engine::pve::PveEngine;
use crate::engine::scoring::{
    apply_tilt_penalty, score_bumper, score_bumper_triangle, timer_bonus,
};
use crate::engine::states::{GamePhase, GameState, TiltEffect};
use crate::player::personnages::character::{Character, select_character};
use crate::player::skills::player_bonus::SkillEffect;

pub struct GameEngine {
    pub state: GameState,
    combo_detector: ComboDetector,
    multiplier: MultiplierState,
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
            pve_engine: PveEngine::new(),
            character: select_character(character_id),
            timer_bonus_given: false,
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
        let event = match envelope.event_type.as_str() {
            "StartGame" => {
                let player_id = envelope
                    .payload
                    .get("player_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_owned();
                GameEvent::StartGame { player_id }
            }
            "EndGame" => GameEvent::EndGame,
            "BallLost" => GameEvent::BallLost,
            "BallSaved" => GameEvent::BallSaved,
            "LifeUp" => GameEvent::LifeUp,
            "UltimateActivated" => {
                let player_id = envelope
                    .payload
                    .get("player_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_owned();
                GameEvent::UltimateActivated { player_id }
            }
            "Bumper" => GameEvent::BumperHit {
                pts: crate::engine::config::BUMPER_SCORE,
            },
            "BumperTriangle" => GameEvent::BumperTriangleHit {
                pts: crate::engine::config::BUMPER_TRIANGLE_SCORE,
            },
            unknown => {
                tracing::debug!(event_type = unknown, "unhandled screen event type");
                return vec![];
            }
        };
        self.process(event)
    }

    pub fn process(&mut self, event: GameEvent) -> Vec<ScreenEnvelope> {
        let now = Instant::now();
        let mut envelopes = Vec::new();

        match event {
            GameEvent::StartGame { .. } => {
                self.state = GameState::new(DEFAULT_LIVES);
                self.state.phase = GamePhase::InGame;
                self.state.session_start = Some(now);
                self.timer_bonus_given = false;
                self.combo_detector = ComboDetector::new();
                self.multiplier = MultiplierState::new();
                self.pve_engine = PveEngine::new();

                let (pve_env, extra) = self.pve_engine.on_event(&event, &mut self.state);
                envelopes.extend(pve_env);
                for e in extra {
                    envelopes.extend(self.process(e));
                }
                envelopes.push(self.emit_score_update());
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
                        self.state.score = apply_tilt_penalty(self.state.score, pts);
                        envelopes.push(self.emit_score_update());
                    }
                    ComboResult::BadgeUnlocked { badge_id } => {
                        envelopes.push(make_event_envelope(
                            "BadgeUnlocked",
                            serde_json::json!({ "badge_id": badge_id }),
                        ));
                    }
                    ComboResult::None => {}
                }
            }

            GameEvent::BumperHit { pts } | GameEvent::BumperTriangleHit { pts } => {
                if self.state.phase != GamePhase::InGame {
                    return envelopes;
                }
                let current_multiplier = self.multiplier.current(now);
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

                envelopes.extend(self.check_timer_bonus(now));
                envelopes.push(self.emit_score_update());
            }

            GameEvent::BumperCombo { count } => {
                if count >= crate::engine::config::MULTIBALL_RING_THRESHOLD {
                    envelopes.extend(self.process(GameEvent::MultiballWin));
                }
            }

            GameEvent::PortalUsed => {
                let pts = crate::engine::scoring::score_portal_bonus();
                self.state.add_score(pts);
                envelopes.push(self.emit_score_update());
            }

            GameEvent::TiltDetected => {
                let effect = self.state.tilt_state.on_tilt();
                match effect {
                    TiltEffect::Penalty(pts) => {
                        self.state.score = apply_tilt_penalty(self.state.score, pts);
                        envelopes.push(self.emit_score_update());
                        envelopes.push(make_event_envelope(
                            "TiltPenalty",
                            serde_json::json!({ "penalty": pts }),
                        ));
                    }
                    TiltEffect::CheatingDetected => {
                        self.state.cheating_detected = true;
                        tracing::warn!("cheating detected — score locked");
                        envelopes.push(make_event_envelope(
                            "CheatingDetected",
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
                envelopes.push(make_event_envelope("MultiballWin", serde_json::Value::Null));
            }

            GameEvent::ScoreMultiplierActivated => {
                let current_multiplier = self.multiplier.current(now);
                envelopes.push(make_event_envelope(
                    "MultiplierUpdate",
                    serde_json::json!({ "multiplier": current_multiplier }),
                ));
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
                self.multiplier.apply(&effect, now);
                self.state.add_score(effect.bonus_pts as u64);
                envelopes.push(self.emit_combo_activated(&effect));
                envelopes.push(self.emit_score_update());
            }

            GameEvent::BossDefeated { boss_id } => {
                tracing::info!(boss_id, "boss defeated event processed");
                envelopes.push(make_event_envelope(
                    "BossDefeated",
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
            self.state.score = timer_bonus(self.state.score, 0);
            return vec![
                make_event_envelope(
                    "TimerBonus",
                    serde_json::json!({ "new_score": self.state.score }),
                ),
                self.emit_score_update(),
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
                    "MultiplierUpdate",
                    serde_json::json!({ "multiplier": factor, "duration_ms": duration_ms }),
                )]
            }
            SkillEffect::AddBalls { count } => vec![make_event_envelope(
                "ExtraBall",
                serde_json::json!({ "count": count }),
            )],
            SkillEffect::ShieldActivated { duration_ms } => vec![make_event_envelope(
                "ShieldActivated",
                serde_json::json!({ "duration_ms": duration_ms }),
            )],
            SkillEffect::AddScore { pts } => {
                self.state.add_score(pts as u64);
                vec![self.emit_score_update()]
            }
            SkillEffect::EmitScreenEvent {
                event_type,
                payload,
            } => {
                vec![make_event_envelope(&event_type, payload)]
            }
            SkillEffect::NoEffect => vec![],
        }
    }

    fn emit_score_update(&self) -> ScreenEnvelope {
        let current_multiplier = self.multiplier.current(Instant::now());
        make_event_envelope(
            "ScoreUpdate",
            serde_json::json!({
                "score": self.state.score,
                "multiplier": current_multiplier,
            }),
        )
    }

    fn emit_life_update(&self) -> ScreenEnvelope {
        make_event_envelope(
            "LifeUpdate",
            serde_json::json!({ "lives_remaining": self.state.lives }),
        )
    }

    fn emit_combo_activated(&self, effect: &crate::combo::ComboEffect) -> ScreenEnvelope {
        make_event_envelope(
            "ComboActivated",
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
            "GameOver",
            serde_json::json!({ "final_score": self.state.score }),
        )
    }
}

fn make_event_envelope(event_type: &str, payload: serde_json::Value) -> ScreenEnvelope {
    ScreenEnvelope {
        from: ScreenId::BackScreen,
        to: ScreenTarget::Broadcast,
        event_type: event_type.to_owned(),
        payload,
    }
}
