use std::fmt;
use std::str::FromStr;

use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use utoipa::ToSchema;

/// Fixed set of known frontend screens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ScreenId {
    FrontScreen,
    BackScreen,
    DmdScreen,
    /// Virtual sender for game-logic events. Never registered as a real screen,
    /// so broadcasts from this id are delivered to all connected screens without exclusion.
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
            "game_engine" => Ok(Self::GameEngine),
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

/// Typed set of events that transit over the screen WebSocket channel.
///
/// All events produced by the game engine or sent by frontend screens are
/// members of this enum. The `Unknown` variant acts as a catch-all so that
/// unknown wire values deserialise gracefully instead of failing.
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
    /// Extension / test events that are not part of the known protocol.
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

/// Envelope for messages transiting between frontend screens via the backend.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, JsonSchema)]
pub struct ScreenEnvelope {
    pub from: ScreenId,
    pub to: ScreenTarget,
    #[schema(value_type = String)]
    pub event_type: ScreenEventType,
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
