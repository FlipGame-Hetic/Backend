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
