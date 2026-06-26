//! Message payloads exchanged between the ESP32 devices, the MQTT bridge, and
//! the central API.
//!
//! Data flows in two directions:
//! - **Inbound** (ESP32 → server): physical inputs and diagnostics.
//! - **Outbound** (server → ESP32): game state updates and commands.
//!
//! [`WsMessage`] is the outer envelope used on the WebSocket between the bridge
//! and the API.  It tags the direction with a `"dir"` field and nests the
//! typed payload inside a `"payload"` object.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::model::{ButtonId, CommandKind, EventKind, GamePhase, HitType};

// ── ESP32 → Server ────────────────────────────────────────────────────────────

/// A physical button was pressed or released.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ButtonInput {
    /// Which button fired.
    pub id: ButtonId,
    /// `1` = pressed, `0` = released.
    pub state: u8,
    /// Milliseconds since device boot.
    pub ts: u64,
}

/// The plunger position changed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PlungerInput {
    /// `1` = pulled back, `0` = released.
    pub state: u8,
    /// Milliseconds since device boot.
    pub ts: u64,
}

/// Raw accelerometer reading from the gyroscope sensor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct GyroInput {
    /// Acceleration on the X axis (m/s²).
    pub ax: f32,
    /// Acceleration on the Y axis (m/s²).
    pub ay: f32,
    /// Acceleration on the Z axis (m/s²).
    pub az: f32,
    /// `true` when the combined acceleration exceeds the tilt threshold.
    pub tilt: bool,
}

/// Periodic health report from the ESP32.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Telemetry {
    /// Wi-Fi signal strength in dBm (negative; closer to 0 is better).
    pub wifi_rssi: i32,
    /// Seconds since the device last booted.
    pub uptime_s: u64,
    /// Main loop frequency in Hz (used to detect performance degradation).
    pub loop_freq_hz: u32,
    /// Free heap memory in bytes.
    pub free_heap: u32,
    /// Number of MQTT reconnections since boot.
    pub mqtt_reconnects: u32,
}

/// A lifecycle event sent by the firmware (boot, OTA, error, etc.).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DeviceEvent {
    /// What happened.
    pub event: EventKind,
    /// Firmware version string at the time of the event.
    pub fw_version: String,
    /// Human-readable reason or detail (e.g. `"power_on"`, `"watchdog"`).
    pub reason: String,
    /// Milliseconds since device boot.
    pub ts: u64,
}

/// Device presence and health snapshot, sent on connect and when status changes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DeviceStatus {
    /// `true` if the device is currently connected and responsive.
    pub online: bool,
    /// Running firmware version.
    pub fw_version: String,
    /// Current IP address on the local network.
    pub ip: String,
    /// Free heap memory in bytes.
    pub free_heap: u32,
    /// One entry per vibration motor — `true` means the motor self-test passed.
    pub vibrators_ok: Vec<bool>,
    /// `true` if the gyroscope self-test passed.
    pub gyro_ok: bool,
}

// ── Frontend Screen → Backend ─────────────────────────────────────────────────

/// Sent by a frontend screen when a physical bumper is hit.
///
/// Transported inside a [`crate::screen::ScreenEnvelope`] with
/// `event_type = "Bumper"` and this struct as the `payload`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BumperHit {
    pub bumper_id: u8,
}

// ── Server → ESP32 ────────────────────────────────────────────────────────────

/// Describes a single collision the server wants the ESP32 to react to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Hit {
    /// Identifier of the object that was hit (e.g. `"bumper-1"`).
    pub id: String,
    /// What kind of object it is (bumper, rail, target…).
    #[serde(rename = "type")]
    pub hit_type: HitType,
    /// Normalised impact force in the range `[0.0, 1.0]`.
    pub force: f32,
}

/// Batch of collision events for one game tick, sent to the ESP32 to trigger
/// vibrations or visual feedback.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BallHit {
    pub hits: Vec<Hit>,
}

/// Current game state pushed to the ESP32 so its displays stay in sync.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct GameState {
    pub state: GamePhase,
    pub ball_number: u32,
    pub score: u64,
    pub player: u32,
    pub total_players: u32,
}

/// A generic command with a free-form `params` payload.
///
/// `params` is an untyped JSON value so new command parameters can be added
/// without changing this struct.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Command {
    pub cmd: CommandKind,
    pub params: serde_json::Value,
}

// ── Unified envelopes ─────────────────────────────────────────────────────────

/// All messages that can travel from an ESP32 to the server.
///
/// The `_type` tag in JSON identifies the variant, so the receiver knows which
/// struct to deserialise into without inspecting the payload fields.
///
/// Example wire form: `{"_type":"Button","id":"L1","state":1,"ts":84200}`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "_type")]
pub enum InboundMessage {
    Button(ButtonInput),
    Plunger(PlungerInput),
    Gyro(GyroInput),
    Telemetry(Telemetry),
    Event(DeviceEvent),
    Status(DeviceStatus),
}

/// All messages that can travel from the server to an ESP32.
///
/// Same `_type` tagging strategy as [`InboundMessage`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "_type")]
pub enum OutboundMessage {
    BallHit(BallHit),
    GameState(GameState),
    Command(Command),
}

/// Outer envelope for all messages on the WebSocket between the bridge and the API.
///
/// `"dir"` in the JSON distinguishes the two directions so a single WebSocket
/// connection can carry traffic both ways:
/// - `"inbound"` → bridge forwarding an ESP32 event to the API
/// - `"outbound"` → API sending a command back to a device via the bridge
///
/// The payload is **nested** (not flattened) to avoid a serde bug (serde#1183)
/// that silently breaks deserialisation when two internally-tagged enums are
/// combined with `#[serde(flatten)]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "dir", rename_all = "snake_case")]
pub enum WsMessage {
    /// Bridge → API: an inbound event from a device.
    Inbound {
        device_id: String,
        payload: InboundMessage,
    },
    /// API → Bridge: an outbound command targeting a device.
    Outbound {
        device_id: String,
        payload: OutboundMessage,
    },
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

    // WsMessage roundtrip tests.
    //
    // These guard against serde#1183: combining `#[serde(flatten)]` inside an
    // internally-tagged enum struct variant with another internally-tagged
    // enum silently breaks deserialization. The fix uses a nested `payload`
    // object instead of flattening, so the wire format must stay nested and
    // every variant must survive a serialize -> assert exact JSON ->
    // deserialize -> assert equality roundtrip.

    /// Assert that `msg` serialises to exactly `expected_json`, that both
    /// `"dir"` and `"_type"` discriminants are present, that `"payload"` is
    /// nested (not flattened), and that the JSON round-trips back to `msg`.
    fn assert_ws_roundtrip(msg: &WsMessage, expected_json: &str) {
        let json = serde_json::to_string(msg).unwrap();
        assert_eq!(json, expected_json, "serialized JSON mismatch");

        // Both internally-tagged discriminants must be present in the wire form.
        assert!(
            json.contains(r#""dir":"#),
            "missing `dir` discriminant: {json}"
        );
        assert!(
            json.contains(r#""_type":"#),
            "missing `_type` discriminant: {json}"
        );

        // The payload must be nested (the serde#1183 regression would flatten it).
        assert!(
            json.contains(r#""payload":{"#),
            "payload is not nested: {json}"
        );

        let parsed: WsMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(&parsed, msg, "roundtrip value mismatch");
    }

    // --- Inbound variants ---

    #[test]
    fn ws_inbound_button_roundtrip() {
        let msg = WsMessage::Inbound {
            device_id: "borne-01".into(),
            payload: InboundMessage::Button(ButtonInput {
                id: ButtonId::L1,
                state: 1,
                ts: 123,
            }),
        };
        assert_ws_roundtrip(
            &msg,
            r#"{"dir":"inbound","device_id":"borne-01","payload":{"_type":"Button","id":"L1","state":1,"ts":123}}"#,
        );
    }

    #[test]
    fn ws_inbound_plunger_roundtrip() {
        let msg = WsMessage::Inbound {
            device_id: "borne-01".into(),
            payload: InboundMessage::Plunger(PlungerInput { state: 0, ts: 456 }),
        };
        assert_ws_roundtrip(
            &msg,
            r#"{"dir":"inbound","device_id":"borne-01","payload":{"_type":"Plunger","state":0,"ts":456}}"#,
        );
    }

    #[test]
    fn ws_inbound_gyro_roundtrip() {
        let msg = WsMessage::Inbound {
            device_id: "borne-01".into(),
            payload: InboundMessage::Gyro(GyroInput {
                ax: 1.0,
                ay: -2.0,
                az: 0.0,
                tilt: false,
            }),
        };
        assert_ws_roundtrip(
            &msg,
            r#"{"dir":"inbound","device_id":"borne-01","payload":{"_type":"Gyro","ax":1.0,"ay":-2.0,"az":0.0,"tilt":false}}"#,
        );
    }

    #[test]
    fn ws_inbound_telemetry_roundtrip() {
        let msg = WsMessage::Inbound {
            device_id: "borne-01".into(),
            payload: InboundMessage::Telemetry(Telemetry {
                wifi_rssi: -50,
                uptime_s: 3600,
                loop_freq_hz: 1000,
                free_heap: 200000,
                mqtt_reconnects: 2,
            }),
        };
        assert_ws_roundtrip(
            &msg,
            r#"{"dir":"inbound","device_id":"borne-01","payload":{"_type":"Telemetry","wifi_rssi":-50,"uptime_s":3600,"loop_freq_hz":1000,"free_heap":200000,"mqtt_reconnects":2}}"#,
        );
    }

    #[test]
    fn ws_inbound_event_roundtrip() {
        let msg = WsMessage::Inbound {
            device_id: "borne-01".into(),
            payload: InboundMessage::Event(DeviceEvent {
                event: EventKind::Boot,
                fw_version: "1.2.3".into(),
                reason: "power_on".into(),
                ts: 789,
            }),
        };
        assert_ws_roundtrip(
            &msg,
            r#"{"dir":"inbound","device_id":"borne-01","payload":{"_type":"Event","event":"boot","fw_version":"1.2.3","reason":"power_on","ts":789}}"#,
        );
    }

    #[test]
    fn ws_inbound_status_roundtrip() {
        let msg = WsMessage::Inbound {
            device_id: "borne-01".into(),
            payload: InboundMessage::Status(DeviceStatus {
                online: true,
                fw_version: "1.2.3".into(),
                ip: "192.168.1.10".into(),
                free_heap: 200000,
                vibrators_ok: vec![true, false],
                gyro_ok: true,
            }),
        };
        assert_ws_roundtrip(
            &msg,
            r#"{"dir":"inbound","device_id":"borne-01","payload":{"_type":"Status","online":true,"fw_version":"1.2.3","ip":"192.168.1.10","free_heap":200000,"vibrators_ok":[true,false],"gyro_ok":true}}"#,
        );
    }

    // --- Outbound variants ---

    #[test]
    fn ws_outbound_ball_hit_roundtrip() {
        let msg = WsMessage::Outbound {
            device_id: "borne-02".into(),
            payload: OutboundMessage::BallHit(BallHit {
                hits: vec![Hit {
                    id: "bumper-1".into(),
                    hit_type: HitType::Bumper,
                    force: 0.75,
                }],
            }),
        };
        assert_ws_roundtrip(
            &msg,
            r#"{"dir":"outbound","device_id":"borne-02","payload":{"_type":"BallHit","hits":[{"id":"bumper-1","type":"bumper","force":0.75}]}}"#,
        );
    }

    #[test]
    fn ws_outbound_game_state_roundtrip() {
        let msg = WsMessage::Outbound {
            device_id: "borne-02".into(),
            payload: OutboundMessage::GameState(GameState {
                state: GamePhase::Playing,
                ball_number: 1,
                score: 12000,
                player: 1,
                total_players: 2,
            }),
        };
        assert_ws_roundtrip(
            &msg,
            r#"{"dir":"outbound","device_id":"borne-02","payload":{"_type":"GameState","state":"playing","ball_number":1,"score":12000,"player":1,"total_players":2}}"#,
        );
    }

    #[test]
    fn ws_outbound_command_roundtrip() {
        let msg = WsMessage::Outbound {
            device_id: "borne-02".into(),
            payload: OutboundMessage::Command(Command {
                cmd: CommandKind::Vibrate,
                params: serde_json::json!({"duration_ms": 200}),
            }),
        };
        assert_ws_roundtrip(
            &msg,
            r#"{"dir":"outbound","device_id":"borne-02","payload":{"_type":"Command","cmd":"vibrate","params":{"duration_ms":200}}}"#,
        );
    }

    // --- Regression: the old flat wire format must no longer deserialize ---

    #[test]
    fn ws_rejects_legacy_flat_format() {
        // Pre-fix wire format flattened `_type` and payload fields next to
        // `dir`/`device_id`. After the fix this must fail to deserialize,
        // proving the format genuinely changed to nested.
        let legacy = r#"{"dir":"inbound","device_id":"abc","_type":"Button","id":"flipper_left","state":1,"ts":123}"#;
        assert!(serde_json::from_str::<WsMessage>(legacy).is_err());
    }
}
