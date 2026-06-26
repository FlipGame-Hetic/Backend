//! MQTT topic parsing and construction.
//!
//! Every MQTT topic in the system follows the pattern
//! `pinball/<device_id>/<subtopic>`.  This module defines [`Topic`] to
//! represent that structure, and [`Subtopic`] for all valid subtopic paths.
use std::fmt;

use thiserror::Error;

/// Prefix that every MQTT topic in the pinball system must start with.
const TOPIC_PREFIX: &str = "pinball";

/// Errors returned when parsing a raw MQTT topic string fails.
#[derive(Debug, Error)]
pub enum TopicError {
    #[error("empty topic string")]
    Empty,
    #[error("missing '{TOPIC_PREFIX}' prefix")]
    MissingPrefix,
    #[error("missing device id segment")]
    MissingDeviceId,
    #[error("unknown subtopic: {0}")]
    UnknownSubtopic(String),
}

/// All valid path segments after `pinball/<device_id>/`.
///
/// | Variant        | MQTT path        | Direction          |
/// |----------------|------------------|--------------------|
/// | InputButton    | `input/button`   | ESP32 → server     |
/// | InputPlunger   | `input/plunger`  | ESP32 → server     |
/// | InputGyro      | `input/gyro`     | ESP32 → server     |
/// | Telemetry      | `telemetry`      | ESP32 → server     |
/// | Events         | `events`         | ESP32 → server     |
/// | Status         | `status`         | ESP32 → server     |
/// | BallHit        | `ball/hit`       | server → ESP32     |
/// | GameState      | `game/state`     | server → ESP32     |
/// | Cmd            | `cmd`            | server → ESP32     |
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Subtopic {
    InputButton,
    InputPlunger,
    InputGyro,
    BallHit,
    GameState,
    Telemetry,
    Events,
    Cmd,
    Status,
}

impl Subtopic {
    fn as_str(&self) -> &'static str {
        match self {
            Self::InputButton => "input/button",
            Self::InputPlunger => "input/plunger",
            Self::InputGyro => "input/gyro",
            Self::BallHit => "ball/hit",
            Self::GameState => "game/state",
            Self::Telemetry => "telemetry",
            Self::Events => "events",
            Self::Cmd => "cmd",
            Self::Status => "status",
        }
    }

    fn from_segments(segments: &[&str]) -> Result<Self, TopicError> {
        match segments {
            ["input", "button"] => Ok(Self::InputButton),
            ["input", "plunger"] => Ok(Self::InputPlunger),
            ["input", "gyro"] => Ok(Self::InputGyro),
            ["ball", "hit"] => Ok(Self::BallHit),
            ["game", "state"] => Ok(Self::GameState),
            ["telemetry"] => Ok(Self::Telemetry),
            ["events"] => Ok(Self::Events),
            ["cmd"] => Ok(Self::Cmd),
            // Accept both "status" and "esp32/status" older firmware versions
            // published on the longer path.
            ["status"] | ["esp32", "status"] => Ok(Self::Status),
            other => {
                let joined = other.join("/");
                Err(TopicError::UnknownSubtopic(joined))
            }
        }
    }
}

/// A parsed MQTT topic in the form `pinball/<device_id>/<subtopic>`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Topic {
    /// The unique identifier of the physical device (e.g. `"esp01"`).
    pub device_id: String,
    /// Which data channel this topic represents.
    pub subtopic: Subtopic,
}

impl Topic {
    /// Build the full MQTT topic string from this struct.
    ///
    /// Example: `Topic { device_id: "esp01", subtopic: Subtopic::InputButton }`
    /// → `"pinball/esp01/input/button"`
    pub fn to_mqtt_topic(&self) -> String {
        format!(
            "{}/{}/{}",
            TOPIC_PREFIX,
            self.device_id,
            self.subtopic.as_str()
        )
    }

    /// Parse a raw MQTT topic string into a [`Topic`].
    ///
    /// Returns an error if the string is empty, missing the `"pinball"` prefix,
    /// missing a device-id segment, or has an unrecognised subtopic path.
    pub fn parse(raw: &str) -> Result<Self, TopicError> {
        if raw.is_empty() {
            return Err(TopicError::Empty);
        }

        let segments: Vec<&str> = raw.split('/').collect();

        if segments.first() != Some(&TOPIC_PREFIX) {
            return Err(TopicError::MissingPrefix);
        }

        let device_id = segments
            .get(1)
            .filter(|s| !s.is_empty())
            .ok_or(TopicError::MissingDeviceId)?;

        let subtopic = Subtopic::from_segments(&segments[2..])?;

        Ok(Self {
            device_id: (*device_id).to_owned(),
            subtopic,
        })
    }

    /// MQTT subscription filter that matches all topics for every device.
    ///
    /// Returns `"pinball/+/#"`, `+` matches exactly one segment (device id),
    /// `#` matches anything that follows (the subtopic path).
    pub fn subscribe_all() -> &'static str {
        "pinball/+/#"
    }

    /// MQTT subscription filter for a single specific device.
    ///
    /// Returns `"pinball/<device_id>/#"`.
    pub fn subscribe_device(device_id: &str) -> String {
        format!("{TOPIC_PREFIX}/{device_id}/#")
    }
}

impl fmt::Display for Topic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_mqtt_topic())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_topics() {
        let cases = vec![
            ("pinball/esp01/input/button", Subtopic::InputButton),
            ("pinball/esp01/input/plunger", Subtopic::InputPlunger),
            ("pinball/esp01/input/gyro", Subtopic::InputGyro),
            ("pinball/esp01/ball/hit", Subtopic::BallHit),
            ("pinball/esp01/game/state", Subtopic::GameState),
            ("pinball/esp01/telemetry", Subtopic::Telemetry),
            ("pinball/esp01/events", Subtopic::Events),
            ("pinball/esp01/cmd", Subtopic::Cmd),
            ("pinball/esp01/status", Subtopic::Status),
        ];

        for (raw, expected_subtopic) in cases {
            let topic =
                Topic::parse(raw).unwrap_or_else(|e| panic!("failed to parse '{raw}': {e}"));
            assert_eq!(topic.device_id, "esp01");
            assert_eq!(topic.subtopic, expected_subtopic);
        }
    }

    #[test]
    fn roundtrip() {
        let topic = Topic {
            device_id: "device_42".to_owned(),
            subtopic: Subtopic::InputButton,
        };
        let raw = topic.to_mqtt_topic();
        let parsed = Topic::parse(&raw).unwrap();
        assert_eq!(topic, parsed);
    }

    #[test]
    fn reject_empty() {
        assert!(matches!(Topic::parse(""), Err(TopicError::Empty)));
    }

    #[test]
    fn reject_bad_prefix() {
        assert!(matches!(
            Topic::parse("other/esp01/telemetry"),
            Err(TopicError::MissingPrefix)
        ));
    }

    #[test]
    fn reject_missing_device_id() {
        assert!(matches!(
            Topic::parse("pinball"),
            Err(TopicError::MissingDeviceId)
        ));
    }

    #[test]
    fn reject_unknown_subtopic() {
        assert!(matches!(
            Topic::parse("pinball/esp01/unknown/path"),
            Err(TopicError::UnknownSubtopic(_))
        ));
    }
}
