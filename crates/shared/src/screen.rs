//! Types for the WebSocket protocol between frontend screens and the backend.
//!
//! The three physical screens (front, back, DMD) each connect over WebSocket.
//! They exchange [`ScreenEnvelope`] messages that carry a typed
//! [`ScreenEventType`] and a free-form JSON payload.
use std::fmt;
use std::str::FromStr;

use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Identifies one of the screens connected to the backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ScreenId {
    /// The main front-facing player display.
    FrontScreen,
    /// The screen on the back of the cabinet.
    BackScreen,
    /// The dot-matrix display (DMD) at the top of the playfield.
    DmdScreen,
    /// Virtual sender used by the game engine.
    ///
    /// Never registered as a real WebSocket connection — it exists only as a
    /// `from` identifier so broadcasts from the game engine reach **all**
    /// connected screens without any exclusion.
    GameEngine,
}

impl ScreenId {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::FrontScreen => "front_screen",
            Self::BackScreen => "back_screen",
            Self::DmdScreen => "dmd_screen",
            Self::GameEngine => "game_engine",
        }
    }

    /// Returns every screen that can physically connect.
    ///
    /// `GameEngine` is intentionally excluded — it is a virtual sender and
    /// never has a real WebSocket session.
    pub fn all() -> &'static [ScreenId] {
        &[Self::FrontScreen, Self::BackScreen, Self::DmdScreen]
    }
}

impl fmt::Display for ScreenId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Returned when a string cannot be parsed into a [`ScreenId`].
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

    /// Accepts both snake_case (`"front_screen"`) and kebab-case
    /// (`"front-screen"`) so the type can be used in URL path segments without
    /// a separate conversion step.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "front_screen" | "front-screen" => Ok(Self::FrontScreen),
            "back_screen" | "back-screen" => Ok(Self::BackScreen),
            "dmd_screen" | "dmd-screen" => Ok(Self::DmdScreen),
            "game_engine" => Ok(Self::GameEngine),
            other => Err(ParseScreenIdError(other.to_owned())),
        }
    }
}

/// Who should receive a screen message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ScreenTarget {
    /// Deliver to one specific screen.
    Screen { id: ScreenId },
    /// Deliver to every screen except the sender.
    Broadcast,
}

/// All event types that can appear in a [`ScreenEnvelope`].
///
/// Split into two groups:
/// - **Inbound** Sent by a screen to the game engine (physical interactions).
/// - **Outbound** Sent by the game engine to screens (game state changes).
///
/// The `Unknown` catch-all lets the system forward unrecognised event strings
/// without crashing, which is useful during development and for custom events
/// in tests.
///
/// ## Why manual Serialize / Deserialize?
///
/// Deriving would produce `{"Unknown": "foo"}` for the `Unknown` variant.
/// Instead we want the wire format to be a plain string in both directions
/// (e.g. `"BossDefeated"` or `"my_custom_event"`), so the impls below
/// serialise via [`as_str`][Self::as_str] and deserialise via [`From<String>`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScreenEventType {
    // Inbound: sent by a screen to the game engine
    StartGame,
    EndGame,
    BallLost,
    BallSaved,
    LifeUp,
    UltimateActivated,
    Bumper,
    BumperTriangle,
    PortalUsed,
    FlipperLeft,
    FlipperRight,
    BallSaverReady,
    MultiballTriggered,
    RailStart,
    RailEnd,
    /// Full hit list from the 3D frontend, forwarded to the ESP32 as `ball/hit`.
    BallHit,
    /// Binary edge: `in_play: true` when ball leaves plunger, `false` on last drain.
    BallInPlay,
    // Outbound: emitted by the game engine to screens
    BossDefeated,
    BossCleared,
    GameOver,
    ScoreUpdate,
    ScoreDelta,
    LifeUpdate,
    ComboActivated,
    BadgeUnlocked,
    MultiballWin,
    MultiplierUpdate,
    TiltPenalty,
    CheatingDetected,
    TimerBonus,
    BossUpdate,
    VictoireFinale,
    EndlessScaling,
    ExtraBall,
    ShieldActivated,
    ExtraFlippers,
    TimeSlowdown,
    Freeze,
    MalusInvisible,
    MalusInkBlot,
    MalusBumperReduction,
    MalusBlackHole,
    MalusModifyBounce,
    MalusStickyBumpers,
    CapacityL2,
    CapacityR2,
    PlungerCharge,
    LeaderboardUpdate,
    MenuButton,
    UltimateTriggered,
    UltimateStopped,
    /// Any event string not listed above — passes through without error.
    Unknown(String),
}

impl ScreenEventType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::StartGame => "StartGame",
            Self::EndGame => "EndGame",
            Self::BallLost => "BallLost",
            Self::BallSaved => "BallSaved",
            Self::LifeUp => "LifeUp",
            Self::UltimateActivated => "UltimateActivated",
            Self::Bumper => "Bumper",
            Self::BumperTriangle => "BumperTriangle",
            Self::PortalUsed => "PortalUsed",
            Self::FlipperLeft => "FlipperLeft",
            Self::FlipperRight => "FlipperRight",
            Self::BallSaverReady => "BallSaverReady",
            Self::MultiballTriggered => "MultiballTriggered",
            Self::RailStart => "RailStart",
            Self::RailEnd => "RailEnd",
            Self::BallHit => "BallHit",
            Self::BallInPlay => "BallInPlay",
            Self::BossDefeated => "BossDefeated",
            Self::BossCleared => "BossCleared",
            Self::GameOver => "GameOver",
            Self::ScoreUpdate => "ScoreUpdate",
            Self::ScoreDelta => "ScoreDelta",
            Self::LifeUpdate => "LifeUpdate",
            Self::ComboActivated => "ComboActivated",
            Self::BadgeUnlocked => "BadgeUnlocked",
            Self::MultiballWin => "MultiballWin",
            Self::MultiplierUpdate => "MultiplierUpdate",
            Self::TiltPenalty => "TiltPenalty",
            Self::CheatingDetected => "CheatingDetected",
            Self::TimerBonus => "TimerBonus",
            Self::BossUpdate => "BossUpdate",
            Self::VictoireFinale => "VictoireFinale",
            Self::EndlessScaling => "EndlessScaling",
            Self::ExtraBall => "ExtraBall",
            Self::ShieldActivated => "ShieldActivated",
            Self::ExtraFlippers => "ExtraFlippers",
            Self::TimeSlowdown => "TimeSlowdown",
            Self::Freeze => "Freeze",
            Self::MalusInvisible => "MalusInvisible",
            Self::MalusInkBlot => "MalusInkBlot",
            Self::MalusBumperReduction => "MalusBumperReduction",
            Self::MalusBlackHole => "MalusBlackHole",
            Self::MalusModifyBounce => "MalusModifyBounce",
            Self::MalusStickyBumpers => "MalusStickyBumpers",
            Self::CapacityL2 => "CapacityL2",
            Self::CapacityR2 => "CapacityR2",
            Self::PlungerCharge => "PlungerCharge",
            Self::LeaderboardUpdate => "LeaderboardUpdate",
            Self::MenuButton => "MenuButton",
            Self::UltimateTriggered => "UltimateTriggered",
            Self::UltimateStopped => "UltimateStopped",
            Self::Unknown(s) => s.as_str(),
        }
    }
}

impl fmt::Display for ScreenEventType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<String> for ScreenEventType {
    fn from(s: String) -> Self {
        match s.as_str() {
            "StartGame" => Self::StartGame,
            "EndGame" => Self::EndGame,
            "BallLost" => Self::BallLost,
            "BallSaved" => Self::BallSaved,
            "LifeUp" => Self::LifeUp,
            "UltimateActivated" => Self::UltimateActivated,
            "Bumper" => Self::Bumper,
            "BumperTriangle" => Self::BumperTriangle,
            "PortalUsed" => Self::PortalUsed,
            "FlipperLeft" => Self::FlipperLeft,
            "FlipperRight" => Self::FlipperRight,
            "BallSaverReady" => Self::BallSaverReady,
            "MultiballTriggered" => Self::MultiballTriggered,
            "RailStart" => Self::RailStart,
            "RailEnd" => Self::RailEnd,
            "BallHit" => Self::BallHit,
            "BallInPlay" => Self::BallInPlay,
            "BossDefeated" => Self::BossDefeated,
            "BossCleared" => Self::BossCleared,
            "GameOver" => Self::GameOver,
            "ScoreUpdate" => Self::ScoreUpdate,
            "ScoreDelta" => Self::ScoreDelta,
            "LifeUpdate" => Self::LifeUpdate,
            "ComboActivated" => Self::ComboActivated,
            "BadgeUnlocked" => Self::BadgeUnlocked,
            "MultiballWin" => Self::MultiballWin,
            "MultiplierUpdate" => Self::MultiplierUpdate,
            "TiltPenalty" => Self::TiltPenalty,
            "CheatingDetected" => Self::CheatingDetected,
            "TimerBonus" => Self::TimerBonus,
            "BossUpdate" => Self::BossUpdate,
            "VictoireFinale" => Self::VictoireFinale,
            "EndlessScaling" => Self::EndlessScaling,
            "ExtraBall" => Self::ExtraBall,
            "ShieldActivated" => Self::ShieldActivated,
            "ExtraFlippers" => Self::ExtraFlippers,
            "TimeSlowdown" => Self::TimeSlowdown,
            "Freeze" => Self::Freeze,
            "MalusInvisible" => Self::MalusInvisible,
            "MalusInkBlot" => Self::MalusInkBlot,
            "MalusBumperReduction" => Self::MalusBumperReduction,
            "MalusBlackHole" => Self::MalusBlackHole,
            "MalusModifyBounce" => Self::MalusModifyBounce,
            "MalusStickyBumpers" => Self::MalusStickyBumpers,
            "CapacityL2" => Self::CapacityL2,
            "CapacityR2" => Self::CapacityR2,
            "PlungerCharge" => Self::PlungerCharge,
            "LeaderboardUpdate" => Self::LeaderboardUpdate,
            "MenuButton" => Self::MenuButton,
            "UltimateTriggered" => Self::UltimateTriggered,
            "UltimateStopped" => Self::UltimateStopped,
            _ => Self::Unknown(s),
        }
    }
}

impl Serialize for ScreenEventType {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ScreenEventType {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Ok(Self::from(s))
    }
}

impl schemars::JsonSchema for ScreenEventType {
    fn schema_name() -> String {
        "ScreenEventType".to_owned()
    }

    fn json_schema(_gen: &mut schemars::r#gen::SchemaGenerator) -> schemars::schema::Schema {
        schemars::schema::SchemaObject {
            instance_type: Some(schemars::schema::InstanceType::String.into()),
            ..Default::default()
        }
        .into()
    }
}

/// A message travelling between frontend screens via the backend WebSocket.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ScreenEnvelope {
    /// Which screen sent this message.
    pub from: ScreenId,
    /// Delivery target: a specific screen or all screens.
    pub to: ScreenTarget,
    /// What kind of event this is.
    pub event_type: ScreenEventType,
    /// Event-specific data.  Structure depends on `event_type`; left untyped
    /// so screens can evolve their payloads independently.
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
    fn screen_event_type_known_roundtrip() {
        let evt = ScreenEventType::BossDefeated;
        let json = serde_json::to_string(&evt).unwrap();
        assert_eq!(json, r#""BossDefeated""#);
        let parsed: ScreenEventType = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, evt);
    }

    #[test]
    fn screen_event_type_unknown_roundtrip() {
        let evt = ScreenEventType::Unknown("custom_event".to_owned());
        let json = serde_json::to_string(&evt).unwrap();
        assert_eq!(json, r#""custom_event""#);
        let parsed: ScreenEventType = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, evt);
    }

    #[test]
    fn screen_envelope_serde_roundtrip() {
        let envelope = ScreenEnvelope {
            from: ScreenId::FrontScreen,
            to: ScreenTarget::Screen {
                id: ScreenId::BackScreen,
            },
            event_type: ScreenEventType::ScoreUpdate,
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
            event_type: ScreenEventType::Unknown("ping".to_owned()),
            payload: serde_json::Value::Null,
        };

        let json = serde_json::to_string(&envelope).unwrap();
        let parsed: ScreenEnvelope = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.from, ScreenId::DmdScreen);
        assert_eq!(parsed.to, ScreenTarget::Broadcast);
    }
}
