use serde::{Deserialize, Serialize};

// Button identifiers (ESP32 GPIO inputs)

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ButtonId {
    FlipperLeft,
    FlipperRight,
    Start,
    Extra1,
    Extra2,
}

// Collision object types

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HitType {
    Bumper,
    Rail,
    Slingshot,
    Drain,
    Target,
    Spinner,
}

// Game state machine phases

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GamePhase {
    Idle,
    Attract,
    Start,
    Playing,
    BallLost,
    Bonus,
    Tilt,
    GameOver,
    HighScore,
}

// Device lifecycle events

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    Boot,
    Ack,
    Alert,
    Error,
    OtaStart,
    OtaDone,
}

// Server => ESP32 command types

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandKind {
    Vibrate,
    Reboot,
    Ota,
    SetConfig,
}
