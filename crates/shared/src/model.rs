use serde::{Deserialize, Serialize};

// Button identifiers (ESP32 GPIO inputs)

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ButtonId {
    #[serde(rename = "L1", alias = "flipper_left")]
    L1,
    #[serde(rename = "R1", alias = "flipper_right")]
    R1,
    #[serde(rename = "L2", alias = "extra1")]
    L2,
    #[serde(rename = "R2", alias = "extra2")]
    R2,
    #[serde(rename = "Start", alias = "start")]
    Start,
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
