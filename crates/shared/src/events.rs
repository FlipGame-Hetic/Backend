use serde::{Deserialize, Serialize};

use crate::model::{ButtonId, CommandKind, EventKind, GamePhase, HitType};

// ESP32 => Server

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ButtonInput {
    pub id: ButtonId,
    pub state: u8,
    pub ts: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlungerInput {
    pub position: f32,
    pub released: bool,
    pub ts: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GyroInput {
    pub ax: f32,
    pub ay: f32,
    pub az: f32,
    pub tilt: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Telemetry {
    pub wifi_rssi: i32,
    pub uptime_s: u64,
    pub loop_freq_hz: u32,
    pub free_heap: u32,
    pub mqtt_reconnects: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceEvent {
    pub event: EventKind,
    pub fw_version: String,
    pub reason: String,
    pub ts: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceStatus {
    pub online: bool,
    pub fw_version: String,
    pub ip: String,
    pub free_heap: u32,
    pub vibrators_ok: Vec<bool>,
    pub gyro_ok: bool,
}

// Frontend Screen => Backend

/// Sent by a frontend screen when a physical bumper is hit.
///
/// Transported inside a [`crate::screen::ScreenEnvelope`] with
/// `event_type = "Bumper"` and this struct as the `payload`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BumperHit {
    pub bumper_id: u8,
}

// Server => ESP32

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hit {
    pub id: String,
    #[serde(rename = "type")]
    pub hit_type: HitType,
    pub force: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BallHit {
    pub hits: Vec<Hit>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameState {
    pub state: GamePhase,
    pub ball_number: u32,
    pub score: u64,
    pub player: u32,
    pub total_players: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Command {
    pub cmd: CommandKind,
    pub params: serde_json::Value,
}

// Unified envelope for all inbound messages

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "_type")]
pub enum InboundMessage {
    Button(ButtonInput),
    Plunger(PlungerInput),
    Gyro(GyroInput),
    Telemetry(Telemetry),
    Event(DeviceEvent),
    Status(DeviceStatus),
}

// Unified envelope for all outbound messages

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "_type")]
pub enum OutboundMessage {
    BallHit(BallHit),
    GameState(GameState),
    Command(Command),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bumper_hit_serializes_correctly() {
        let hit = BumperHit { bumper_id: 10 };
        let json = serde_json::to_string(&hit).unwrap();
        assert_eq!(json, r#"{"bumper_id":10}"#);
    }

    #[test]
    fn bumper_hit_deserializes_correctly() {
        let json = r#"{"bumper_id":10}"#;
        let hit: BumperHit = serde_json::from_str(json).unwrap();
        assert_eq!(hit, BumperHit { bumper_id: 10 });
    }

    #[test]
    fn bumper_hit_roundtrip() {
        let hit = BumperHit { bumper_id: 255 };
        let json = serde_json::to_string(&hit).unwrap();
        let parsed: BumperHit = serde_json::from_str(&json).unwrap();
        assert_eq!(hit, parsed);
    }

    #[test]
    fn bumper_hit_rejects_out_of_range_payload() {
        // u8 max is 255, values > 255 should fail deserialization
        let json = r#"{"bumper_id":256}"#;
        assert!(serde_json::from_str::<BumperHit>(json).is_err());
    }
}

// WebSocket envelope (Bridge ↔ API)

// Every message transiting over the WebSocket between a borne's bridge
// and the central API is wrapped in this envelope so the API knows
// which device it comes from / should be routed to.

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "dir", rename_all = "snake_case")]
pub enum WsMessage {
    /// Bridge => API: an inbound event from a device.
    Inbound {
        device_id: String,
        #[serde(flatten)]
        payload: InboundMessage,
    },
    /// API => Bridge: an outbound command targeting a device.
    Outbound {
        device_id: String,
        #[serde(flatten)]
        payload: OutboundMessage,
    },
}
