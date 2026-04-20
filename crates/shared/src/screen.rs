use std::fmt;
use std::str::FromStr;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Fixed set of known frontend screens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ScreenId {
    FrontScreen,
    BackScreen,
    DmdScreen,
}

impl ScreenId {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::FrontScreen => "front_screen",
            Self::BackScreen => "back_screen",
            Self::DmdScreen => "dmd_screen",
        }
    }

    /// Returns all known screen variants (useful for iteration / validation).
    pub fn all() -> &'static [ScreenId] {
        &[Self::FrontScreen, Self::BackScreen, Self::DmdScreen]
    }
}

impl fmt::Display for ScreenId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseScreenIdError(String);

impl fmt::Display for ParseScreenIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown screen id: '{}'", self.0)
    }
}

impl std::error::Error for ParseScreenIdError {}

impl FromStr for ScreenId {
    type Err = ParseScreenIdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "front_screen" | "front-screen" => Ok(Self::FrontScreen),
            "back_screen" | "back-screen" => Ok(Self::BackScreen),
            "dmd_screen" | "dmd-screen" => Ok(Self::DmdScreen),
            other => Err(ParseScreenIdError(other.to_owned())),
        }
    }
}

/// Routing target for a screen message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ScreenTarget {
    /// Send to a single specific screen.
    Screen { id: ScreenId },
    /// Send to all screens except the sender.
    Broadcast,
}

/// Envelope for messages transiting between frontend screens via the backend.
///
/// The `event_type` field is a free-form string that identifies the kind of event
/// (e.g. "game_state_update", "score_change"). The `payload` is intentionally
/// kept as `serde_json::Value` to stay flexible — strong typing will be layered
/// on top when the processing pipeline is implemented.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, JsonSchema)]
pub struct ScreenEnvelope {
    pub from: ScreenId,
    pub to: ScreenTarget,
    pub event_type: String,
    pub payload: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screen_id_roundtrip_str() {
        for id in ScreenId::all() {
            let s = id.as_str();
            let parsed: ScreenId = s.parse().unwrap();
            assert_eq!(*id, parsed);
        }
    }

    #[test]
    fn screen_id_accepts_kebab_case() {
        assert_eq!(
            "front-screen".parse::<ScreenId>().unwrap(),
            ScreenId::FrontScreen
        );
        assert_eq!(
            "back-screen".parse::<ScreenId>().unwrap(),
            ScreenId::BackScreen
        );
        assert_eq!(
            "dmd-screen".parse::<ScreenId>().unwrap(),
            ScreenId::DmdScreen
        );
    }

    #[test]
    fn screen_id_rejects_unknown() {
        assert!("unknown".parse::<ScreenId>().is_err());
    }

    #[test]
    fn screen_id_serde_json_roundtrip() {
        let id = ScreenId::FrontScreen;
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, r#""front_screen""#);

        let parsed: ScreenId = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, id);
    }

    #[test]
    fn screen_target_serde_broadcast() {
        let target = ScreenTarget::Broadcast;
        let json = serde_json::to_string(&target).unwrap();
        let parsed: ScreenTarget = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, target);
    }

    #[test]
    fn screen_target_serde_specific() {
        let target = ScreenTarget::Screen {
            id: ScreenId::BackScreen,
        };
        let json = serde_json::to_string(&target).unwrap();
        assert!(json.contains("back_screen"));

        let parsed: ScreenTarget = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, target);
    }

    #[test]
    fn screen_envelope_serde_roundtrip() {
        let envelope = ScreenEnvelope {
            from: ScreenId::FrontScreen,
            to: ScreenTarget::Screen {
                id: ScreenId::BackScreen,
            },
            event_type: "game_state_update".to_owned(),
            payload: serde_json::json!({ "score": 42 }),
        };

        let json = serde_json::to_string(&envelope).unwrap();
        let parsed: ScreenEnvelope = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.from, envelope.from);
        assert_eq!(parsed.to, envelope.to);
        assert_eq!(parsed.event_type, envelope.event_type);
        assert_eq!(parsed.payload, envelope.payload);
    }

    #[test]
    fn screen_envelope_serde_broadcast() {
        let envelope = ScreenEnvelope {
            from: ScreenId::DmdScreen,
            to: ScreenTarget::Broadcast,
            event_type: "ping".to_owned(),
            payload: serde_json::Value::Null,
        };

        let json = serde_json::to_string(&envelope).unwrap();
        let parsed: ScreenEnvelope = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.from, ScreenId::DmdScreen);
        assert_eq!(parsed.to, ScreenTarget::Broadcast);
    }
}
