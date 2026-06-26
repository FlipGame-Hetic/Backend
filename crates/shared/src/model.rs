//! Core enums used across the pinball backend.
//!
//! These types are shared between the MQTT bridge, the game engine, and the
//! screen protocol.  They all derive `Serialize`/`Deserialize` so they can
//! travel over MQTT or WebSocket without conversion.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Identifies a physical button on the pinball machine.
///
/// The `alias` attributes accept the older firmware naming (`"flipper_left"`,
/// `"flipper_right"`, etc.) so both old and new firmware can be handled without
/// a migration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub enum ButtonId {
    /// Left flipper button.
    #[serde(rename = "L1", alias = "flipper_left")]
    L1,
    /// Right flipper button.
    #[serde(rename = "R1", alias = "flipper_right")]
    R1,
    /// Secondary left button (extra action).
    #[serde(rename = "L2", alias = "extra1")]
    L2,
    /// Secondary right button (extra action).
    #[serde(rename = "R2", alias = "extra2")]
    R2,
    /// Start / player-add button.
    #[serde(rename = "Start", alias = "start")]
    Start,
    /// Micro-switch triggered when the plunger is pulled back past the lane
    /// entry point.
    #[serde(rename = "under_plunger")]
    UnderPlunger,
    /// Top button on the front panel.
    #[serde(rename = "top")]
    Top,
    /// Middle button on the front panel.
    #[serde(rename = "middle")]
    Middle,
    /// Bottom button on the front panel.
    #[serde(rename = "bottom")]
    Bottom,
}

/// The type of object the ball collided with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HitType {
    /// Round pop bumper.
    Bumper,
    /// Side rail / wall.
    Rail,
    /// Slingshot rubber band.
    Slingshot,
    /// Ball fell into the drain (end of ball).
    Drain,
    /// Drop or stand-up target.
    Target,
    /// Spinning target.
    Spinner,
}

/// State machine phases for a pinball game session.
///
/// The typical lifecycle is:
/// `Idle` → `Attract` → `Start` → `Playing` ↔ `BallLost` → `Bonus` → `GameOver` / `HighScore`
/// `Tilt` can be entered from `Playing` at any point.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GamePhase {
    /// No active session; machine is waiting for a coin/credit.
    Idle,
    /// Demo / attract mode playing on the screens.
    Attract,
    /// Game is initialising (countdown, ball serving).
    Start,
    /// Ball is in play.
    Playing,
    /// Ball drained; waiting to serve the next ball.
    BallLost,
    /// End-of-ball bonus countdown.
    Bonus,
    /// Machine was physically tilted; session penalised.
    Tilt,
    /// All balls played; session ended.
    GameOver,
    /// Player is entering their name for the high-score table.
    HighScore,
}

/// Lifecycle events reported by the ESP32 firmware.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    /// Device just finished booting.
    Boot,
    /// Device acknowledged a command from the server.
    Ack,
    /// Non-fatal warning (e.g. sensor read out of range).
    Alert,
    /// Firmware-level error (logged but device stays online).
    Error,
    /// OTA firmware update has started.
    OtaStart,
    /// OTA firmware update finished successfully.
    OtaDone,
}

/// Commands the server can send to an ESP32 device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CommandKind {
    /// Fire one or more vibration motors.
    Vibrate,
    /// Reboot the device.
    Reboot,
    /// Start an over-the-air firmware update.
    Ota,
    /// Push a new runtime configuration to the device.
    SetConfig,
}
