use serde::{Deserialize, Serialize};

use crate::model::{ButtonId, CommandKind, EventKind, GamePhase, HitType};

// ESP32 => Server

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ButtonInput {
    pub id: ButtonId,
    pub state: u8,
    pub ts: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlungerInput {
    pub position: f32,
    pub released: bool,
    pub ts: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GyroInput {
    pub ax: f32,
    pub ay: f32,
    pub az: f32,
    pub tilt: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Telemetry {
    pub wifi_rssi: i32,
    pub uptime_s: u64,
    pub loop_freq_hz: u32,
    pub free_heap: u32,
    pub mqtt_reconnects: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeviceEvent {
    pub event: EventKind,
    pub fw_version: String,
    pub reason: String,
    pub ts: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hit {
    pub id: String,
    #[serde(rename = "type")]
    pub hit_type: HitType,
    pub force: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BallHit {
    pub hits: Vec<Hit>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameState {
    pub state: GamePhase,
    pub ball_number: u32,
    pub score: u64,
    pub player: u32,
    pub total_players: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Command {
    pub cmd: CommandKind,
    pub params: serde_json::Value,
}

// Unified envelope for all inbound messages

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

    // WsMessage roundtrip tests.
    //
    // These guard against serde#1183: combining `#[serde(flatten)]` inside an
    // internally-tagged enum struct variant with another internally-tagged
    // enum silently breaks deserialization. The fix uses a nested `payload`
    // object instead of flattening, so the wire format must stay nested and
    // every variant must survive a serialize -> assert exact JSON ->
    // deserialize -> assert equality roundtrip.

    /// Asserts that `msg` serializes to exactly `expected_json`, that the
    /// `dir` and `_type` discriminants are present, and that the JSON
    /// deserializes back to a value equal to `msg`.
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
            payload: InboundMessage::Plunger(PlungerInput {
                position: 0.5,
                released: true,
                ts: 456,
            }),
        };
        assert_ws_roundtrip(
            &msg,
            r#"{"dir":"inbound","device_id":"borne-01","payload":{"_type":"Plunger","position":0.5,"released":true,"ts":456}}"#,
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

// WebSocket envelope (Bridge ↔ API)

// Every message transiting over the WebSocket between a borne's bridge
// and the central API is wrapped in this envelope so the API knows
// which device it comes from / should be routed to.

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "dir", rename_all = "snake_case")]
pub enum WsMessage {
    /// Bridge => API: an inbound event from a device.
    Inbound {
        device_id: String,
        payload: InboundMessage,
    },
    /// API => Bridge: an outbound command targeting a device.
    Outbound {
        device_id: String,
        payload: OutboundMessage,
    },
}
