use std::time::Instant;

use shared::events::InboundMessage;
use shared::model::ButtonId;
use shared::screen::{ScreenEnvelope, ScreenEventType};

use crate::engine::config;
use crate::engine::events::{ButtonSide, GameEvent};
use crate::engine::states::GamePhase;

use super::{GameEngine, make_event_envelope};

impl GameEngine {
    /// Translate a raw hardware message (button, gyro, plunger) into `GameEvent`s
    /// and forward them to `process`.
    pub fn handle_inbound(&mut self, msg: &InboundMessage) -> Vec<ScreenEnvelope> {
        match msg {
            InboundMessage::Button(btn) => {
                if let Some(side) = ButtonSide::from_button_id(&btn.id) {
                    let event_type = match &side {
                        ButtonSide::Left => ScreenEventType::FlipperLeft,
                        ButtonSide::Right => ScreenEventType::FlipperRight,
                    };
                    let mut envelopes = vec![make_event_envelope(
                        event_type,
                        serde_json::json!({ "state": btn.state }),
                    )];
                    if btn.state != 0 {
                        envelopes.extend(self.process(GameEvent::ButtonPressed { side }));
                    }
                    return envelopes;
                }

                if self.state.phase == GamePhase::InGame {
                    match btn.id {
                        ButtonId::L2 | ButtonId::R2 if btn.state > 0 => {
                            let now = Instant::now();
                            return self.process_ulti_press(now);
                        }
                        ButtonId::UnderPlunger => {
                            let mut envelopes = vec![make_event_envelope(
                                ScreenEventType::PlungerCharge,
                                serde_json::json!({ "state": btn.state }),
                            )];
                            if btn.state == 0 {
                                envelopes.extend(self.process(GameEvent::BallLaunched));
                            }
                            return envelopes;
                        }
                        _ => {}
                    }
                }

                vec![]
            }
            InboundMessage::Gyro(gyro) if gyro.tilt => self.process(GameEvent::TiltDetected),
            InboundMessage::Plunger(plunger) if plunger.state == 0 => {
                self.process(GameEvent::BallLaunched)
            }
            _ => vec![],
        }
    }

    /// Translate a screen event (from the Unity frontend) into a `GameEvent` and process it.
    pub fn handle_screen_event(&mut self, envelope: &ScreenEnvelope) -> Vec<ScreenEnvelope> {
        let event = match &envelope.event_type {
            ScreenEventType::StartGame => GameEvent::StartGame,
            ScreenEventType::EndGame => GameEvent::EndGame,
            ScreenEventType::BallLost => GameEvent::BallLost,
            ScreenEventType::BallSaved => GameEvent::BallSaved,
            ScreenEventType::LifeUp => GameEvent::LifeUp,
            // UltimateActivated is no longer the activation path.
            // L2/R2 is the authoritative trigger. Ignore this event to avoid the old ping-pong.
            ScreenEventType::UltimateActivated => return vec![],
            ScreenEventType::CapacityL2 | ScreenEventType::CapacityR2 => {
                return self.process_ulti_press(Instant::now());
            }
            ScreenEventType::Bumper => {
                let ball_id = envelope
                    .payload
                    .get("ball_id")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                GameEvent::BumperHit {
                    pts: config::get().bumper_score,
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
                    pts: config::get().bumper_triangle_score,
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
}

#[cfg(test)]
mod tests {
    use shared::events::{ButtonInput, GyroInput, InboundMessage, PlungerInput};
    use shared::model::ButtonId;
    use shared::screen::{ScreenEnvelope, ScreenEventType, ScreenId, ScreenTarget};

    use crate::engine::events::GameEvent;
    use crate::engine::states::GamePhase;

    use super::GameEngine;

    fn started() -> GameEngine {
        let mut e = GameEngine::new("enforcer");
        e.process(GameEvent::StartGame);
        e
    }

    fn btn(id: ButtonId, state: u8) -> InboundMessage {
        InboundMessage::Button(ButtonInput { id, state, ts: 0 })
    }

    fn screen_ev(event_type: ScreenEventType) -> ScreenEnvelope {
        ScreenEnvelope {
            from: ScreenId::GameEngine,
            to: ScreenTarget::Broadcast,
            event_type,
            payload: serde_json::json!({}),
        }
    }

    // handle_inbound: flipper buttons

    #[test]
    fn l1_press_emits_flipper_left() {
        let mut engine = started();
        let evs = engine.handle_inbound(&btn(ButtonId::L1, 1));
        assert!(
            evs.iter()
                .any(|e| e.event_type == ScreenEventType::FlipperLeft)
        );
    }

    #[test]
    fn r1_press_emits_flipper_right() {
        let mut engine = started();
        let evs = engine.handle_inbound(&btn(ButtonId::R1, 1));
        assert!(
            evs.iter()
                .any(|e| e.event_type == ScreenEventType::FlipperRight)
        );
    }

    #[test]
    fn l1_release_emits_flipper_left_but_no_button_pressed_event() {
        let mut engine = started();
        // state=0 → release; should not trigger ButtonPressed (no combo push)
        let evs = engine.handle_inbound(&btn(ButtonId::L1, 0));
        assert!(
            evs.iter()
                .any(|e| e.event_type == ScreenEventType::FlipperLeft)
        );
        // No ScoreUpdate/ScoreDelta from combo processing
        assert!(
            !evs.iter()
                .any(|e| e.event_type == ScreenEventType::ScoreUpdate)
        );
    }

    // handle_inbound: ulti (L2/R2)

    #[test]
    fn l2_press_during_game_triggers_ulti_path() {
        let mut engine = started();
        // Charge to full so ulti actually fires
        let charge_max = engine.character.stats().charge_profile.charge_max;
        engine.state.ultimate_charge = charge_max;
        let evs = engine.handle_inbound(&btn(ButtonId::L2, 1));
        assert!(
            evs.iter()
                .any(|e| e.event_type == ScreenEventType::UltimateTriggered)
        );
    }

    #[test]
    fn r2_press_during_game_triggers_ulti_path() {
        let mut engine = started();
        let charge_max = engine.character.stats().charge_profile.charge_max;
        engine.state.ultimate_charge = charge_max;
        let evs = engine.handle_inbound(&btn(ButtonId::R2, 1));
        assert!(
            evs.iter()
                .any(|e| e.event_type == ScreenEventType::UltimateTriggered)
        );
    }

    #[test]
    fn l2_press_outside_in_game_returns_empty() {
        let mut engine = GameEngine::new("enforcer");
        // Phase is Idle
        let evs = engine.handle_inbound(&btn(ButtonId::L2, 1));
        assert!(evs.is_empty());
    }

    #[test]
    fn l2_release_does_not_trigger_ulti() {
        let mut engine = started();
        let charge_max = engine.character.stats().charge_profile.charge_max;
        engine.state.ultimate_charge = charge_max;
        // state=0 → release, should be ignored
        let evs = engine.handle_inbound(&btn(ButtonId::L2, 0));
        assert!(
            !evs.iter()
                .any(|e| e.event_type == ScreenEventType::UltimateTriggered)
        );
    }

    // handle_inbound: plunger

    #[test]
    fn plunger_held_emits_plunger_charge_during_game() {
        let mut engine = started();
        let evs = engine.handle_inbound(&btn(ButtonId::UnderPlunger, 1));
        assert!(
            evs.iter()
                .any(|e| e.event_type == ScreenEventType::PlungerCharge)
        );
    }

    #[test]
    fn plunger_released_emits_plunger_charge_and_ball_launched() {
        let mut engine = started();
        // state=0 → release fires BallLaunched (no-op in process, but the inbound path runs)
        let evs = engine.handle_inbound(&btn(ButtonId::UnderPlunger, 0));
        assert!(
            evs.iter()
                .any(|e| e.event_type == ScreenEventType::PlungerCharge)
        );
    }

    #[test]
    fn plunger_outside_in_game_returns_empty() {
        let mut engine = GameEngine::new("enforcer");
        let evs = engine.handle_inbound(&btn(ButtonId::UnderPlunger, 0));
        assert!(evs.is_empty());
    }

    // handle_inbound: gyro

    #[test]
    fn gyro_tilt_triggers_tilt_detected() {
        let mut engine = started();
        let msg = InboundMessage::Gyro(GyroInput {
            ax: 0.0,
            ay: 0.0,
            az: 0.0,
            tilt: true,
        });
        let evs = engine.handle_inbound(&msg);
        assert!(
            evs.iter()
                .any(|e| e.event_type == ScreenEventType::TiltPenalty)
        );
    }

    #[test]
    fn gyro_no_tilt_returns_empty() {
        let mut engine = started();
        let msg = InboundMessage::Gyro(GyroInput {
            ax: 0.0,
            ay: 0.0,
            az: 0.0,
            tilt: false,
        });
        let evs = engine.handle_inbound(&msg);
        assert!(evs.is_empty());
    }

    // handle_inbound: plunger (direct InboundMessage::Plunger)

    #[test]
    fn plunger_inbound_release_processes_ball_launched() {
        let mut engine = started();
        let msg = InboundMessage::Plunger(PlungerInput { state: 0, ts: 0 });
        // BallLaunched is a no-op in process but must not panic
        let evs = engine.handle_inbound(&msg);
        assert!(evs.is_empty());
    }

    #[test]
    fn plunger_inbound_held_returns_empty() {
        let mut engine = started();
        let msg = InboundMessage::Plunger(PlungerInput { state: 1, ts: 0 });
        let evs = engine.handle_inbound(&msg);
        assert!(evs.is_empty());
    }

    // handle_screen_event

    #[test]
    fn screen_start_game_sets_in_game() {
        let mut engine = GameEngine::new("enforcer");
        engine.handle_screen_event(&screen_ev(ScreenEventType::StartGame));
        assert_eq!(engine.state.phase, GamePhase::InGame);
    }

    #[test]
    fn screen_end_game_sets_game_over() {
        let mut engine = started();
        engine.handle_screen_event(&screen_ev(ScreenEventType::EndGame));
        assert_eq!(engine.state.phase, GamePhase::GameOver);
    }

    #[test]
    fn screen_ball_lost_decrements_lives() {
        let mut engine = started();
        let before = engine.state.lives;
        engine.handle_screen_event(&screen_ev(ScreenEventType::BallLost));
        assert_eq!(engine.state.lives, before - 1);
    }

    #[test]
    fn screen_ball_saved_is_no_op() {
        let mut engine = started();
        let evs = engine.handle_screen_event(&screen_ev(ScreenEventType::BallSaved));
        assert!(evs.is_empty());
    }

    #[test]
    fn screen_life_up_increments_lives() {
        let mut engine = started();
        let before = engine.state.lives;
        engine.handle_screen_event(&screen_ev(ScreenEventType::LifeUp));
        assert_eq!(engine.state.lives, before + 1);
    }

    #[test]
    fn screen_ultimate_activated_is_ignored() {
        let mut engine = started();
        let before = engine.state.score;
        let evs = engine.handle_screen_event(&screen_ev(ScreenEventType::UltimateActivated));
        assert!(evs.is_empty());
        assert_eq!(engine.state.score, before);
    }

    #[test]
    fn screen_capacity_l2_triggers_ulti_path() {
        let mut engine = started();
        let charge_max = engine.character.stats().charge_profile.charge_max;
        engine.state.ultimate_charge = charge_max;
        let evs = engine.handle_screen_event(&screen_ev(ScreenEventType::CapacityL2));
        assert!(
            evs.iter()
                .any(|e| e.event_type == ScreenEventType::UltimateTriggered)
        );
    }

    #[test]
    fn screen_bumper_adds_score() {
        let mut engine = started();
        let before = engine.state.score;
        engine.handle_screen_event(&screen_ev(ScreenEventType::Bumper));
        assert!(engine.state.score > before);
    }

    #[test]
    fn screen_bumper_with_ball_id_propagates() {
        let mut engine = started();
        let mut env = screen_ev(ScreenEventType::Bumper);
        env.payload = serde_json::json!({ "ball_id": "ball-42" });
        let evs = engine.handle_screen_event(&env);
        let update = evs
            .iter()
            .find(|e| e.event_type == ScreenEventType::ScoreUpdate)
            .unwrap();
        assert_eq!(update.payload["ball_id"], serde_json::json!("ball-42"));
    }

    #[test]
    fn screen_bumper_triangle_scores_more_than_regular() {
        let score_for = |ev_type: ScreenEventType| {
            let mut e = started();
            let before = e.state.score;
            e.handle_screen_event(&screen_ev(ev_type));
            e.state.score - before
        };
        assert!(score_for(ScreenEventType::BumperTriangle) > score_for(ScreenEventType::Bumper));
    }

    #[test]
    fn screen_portal_used_adds_score() {
        let mut engine = started();
        let before = engine.state.score;
        engine.handle_screen_event(&screen_ev(ScreenEventType::PortalUsed));
        assert!(engine.state.score > before);
    }

    #[test]
    fn screen_flipper_left_emits_no_crash() {
        let mut engine = started();
        // ButtonPressed in InGame with no combo match → ComboResult::None → no envelopes
        let evs = engine.handle_screen_event(&screen_ev(ScreenEventType::FlipperLeft));
        // just ensure it doesn't panic; combo result may be empty
        let _ = evs;
    }

    #[test]
    fn screen_ball_saver_ready_adds_score() {
        let mut engine = started();
        let before = engine.state.score;
        engine.handle_screen_event(&screen_ev(ScreenEventType::BallSaverReady));
        assert!(engine.state.score > before);
    }

    #[test]
    fn screen_multiball_triggered_scores() {
        let mut engine = started();
        let before = engine.state.score;
        engine.handle_screen_event(&screen_ev(ScreenEventType::MultiballTriggered));
        assert!(engine.state.score > before);
    }

    #[test]
    fn screen_unknown_event_returns_empty() {
        let mut engine = started();
        // GameOver is not handled by handle_screen_event → falls through to "other"
        let evs = engine.handle_screen_event(&screen_ev(ScreenEventType::GameOver));
        assert!(evs.is_empty());
    }
}
