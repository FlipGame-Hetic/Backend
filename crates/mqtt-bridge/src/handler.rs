use shared::dto::{Subtopic, Topic, TopicError};
use shared::events::{
    ButtonInput, DeviceEvent, DeviceStatus, GyroInput, InboundMessage, PlungerInput, Telemetry,
};
use tracing::{debug, trace, warn};

use crate::errors::{BridgeError, Result};

/// Outcome of handling a single MQTT message.
#[derive(Debug)]
pub struct Handled {
    pub device_id: String,
    pub message: InboundMessage,
}

/// Route an incoming MQTT publish to the correct deserializer based on its topic.
pub fn handle_publish(topic_str: &str, payload: &[u8]) -> Result<Handled> {
    let topic = Topic::parse(topic_str)?;

    trace!(
        topic = %topic,
        device_id = %topic.device_id,
        payload_len = payload.len(),
        "routing message"
    );

    let message = deserialize(&topic.subtopic, payload)?;

    debug!(
        device_id = %topic.device_id,
        subtopic = ?topic.subtopic,
        "handled message"
    );

    Ok(Handled {
        device_id: topic.device_id,
        message,
    })
}

fn deserialize(subtopic: &Subtopic, payload: &[u8]) -> Result<InboundMessage> {
    match subtopic {
        Subtopic::InputButton => {
            let input: ButtonInput = serde_json::from_slice(payload)?;
            Ok(InboundMessage::Button(input))
        }
        Subtopic::InputPlunger => {
            let input: PlungerInput = serde_json::from_slice(payload)?;
            Ok(InboundMessage::Plunger(input))
        }
        Subtopic::InputGyro => {
            let input: GyroInput = serde_json::from_slice(payload)?;
            Ok(InboundMessage::Gyro(input))
        }
        Subtopic::Telemetry => {
            let input: Telemetry = serde_json::from_slice(payload)?;
            Ok(InboundMessage::Telemetry(input))
        }
        Subtopic::Events => {
            let input: DeviceEvent = serde_json::from_slice(payload)?;
            Ok(InboundMessage::Event(input))
        }
        Subtopic::Status => {
            let input: DeviceStatus = serde_json::from_slice(payload)?;
            Ok(InboundMessage::Status(input))
        }
        // Server → ESP32 topics are outbound-only, we don't deserialize them here.
        Subtopic::BallHit | Subtopic::GameState | Subtopic::Cmd => {
            warn!(subtopic = ?subtopic, "received message on outbound-only topic, ignoring");
            Err(BridgeError::Topic(TopicError::UnknownSubtopic(format!(
                "{subtopic:?} is outbound-only"
            ))))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::model::ButtonId;

    #[test]
    fn handle_button_press() {
        let topic = "pinball/esp01/input/button";
        let payload = br#"{"id":"flipper_left","state":1,"ts":84200}"#;

        let handled = handle_publish(topic, payload).unwrap();
        assert_eq!(handled.device_id, "esp01");

        match handled.message {
            InboundMessage::Button(btn) => {
                assert_eq!(btn.id, ButtonId::FlipperLeft);
                assert_eq!(btn.state, 1);
                assert_eq!(btn.ts, 84200);
            }
            other => panic!("expected Button, got {other:?}"),
        }
    }

    #[test]
    fn handle_plunger_release() {
        let topic = "pinball/esp01/input/plunger";
        let payload = br#"{"position":0.82,"released":true,"ts":84450}"#;

        let handled = handle_publish(topic, payload).unwrap();

        match handled.message {
            InboundMessage::Plunger(p) => {
                assert!(p.released);
                assert!((p.position - 0.82).abs() < f32::EPSILON);
            }
            other => panic!("expected Plunger, got {other:?}"),
        }
    }

    #[test]
    fn handle_gyro() {
        let topic = "pinball/dev42/input/gyro";
        let payload = br#"{"ax":1.85,"ay":2.10,"az":9.40,"tilt":true}"#;

        let handled = handle_publish(topic, payload).unwrap();
        assert_eq!(handled.device_id, "dev42");

        match handled.message {
            InboundMessage::Gyro(g) => assert!(g.tilt),
            other => panic!("expected Gyro, got {other:?}"),
        }
    }

    #[test]
    fn handle_telemetry() {
        let topic = "pinball/esp01/telemetry";
        let payload = br#"{"wifi_rssi":-42,"uptime_s":84200,"loop_freq_hz":1000,"free_heap":142000,"mqtt_reconnects":0}"#;

        let handled = handle_publish(topic, payload).unwrap();

        match handled.message {
            InboundMessage::Telemetry(t) => {
                assert_eq!(t.wifi_rssi, -42);
                assert_eq!(t.free_heap, 142000);
            }
            other => panic!("expected Telemetry, got {other:?}"),
        }
    }

    #[test]
    fn handle_device_event() {
        let topic = "pinball/esp01/events";
        let payload =
            br#"{"event":"boot","fw_version":"1.2.0","reason":"power_on","ts":1719312000000}"#;

        let handled = handle_publish(topic, payload).unwrap();

        match handled.message {
            InboundMessage::Event(e) => {
                assert_eq!(e.event, shared::model::EventKind::Boot);
            }
            other => panic!("expected Event, got {other:?}"),
        }
    }

    #[test]
    fn handle_device_status() {
        let topic = "pinball/esp01/status";
        let payload = br#"{"online":true,"fw_version":"1.2.0","ip":"192.168.1.50","free_heap":142000,"vibrators_ok":[true,true,true,true,true,true,true,true,true],"gyro_ok":true}"#;

        let handled = handle_publish(topic, payload).unwrap();

        match handled.message {
            InboundMessage::Status(s) => {
                assert!(s.online);
                assert_eq!(s.vibrators_ok.len(), 9);
            }
            other => panic!("expected Status, got {other:?}"),
        }
    }

    #[test]
    fn reject_outbound_topic() {
        let topic = "pinball/esp01/ball/hit";
        let payload = br#"{"hits":[]}"#;

        let result = handle_publish(topic, payload);
        assert!(result.is_err());
    }

    #[test]
    fn reject_malformed_payload() {
        let topic = "pinball/esp01/input/button";
        let payload = br#"{"not_valid": true}"#;

        let result = handle_publish(topic, payload);
        assert!(result.is_err());
    }

    #[test]
    fn reject_invalid_topic() {
        let topic = "invalid/path";
        let payload = b"{}";

        let result = handle_publish(topic, payload);
        assert!(result.is_err());
    }
}
