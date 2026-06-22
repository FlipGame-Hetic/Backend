use lucyd::lucy_mqtt;
use shared::events::{
    BallHit, ButtonInput, Command, DeviceEvent, DeviceStatus, GameState, GyroInput, PlungerInput,
    Telemetry,
};

// Virtual documentation functions — never called at runtime.
// They exist solely to register MQTT topics in the Lucyd registry at link time
// via inventory::submit!, so the /docs UI can display them.
//
// The mqtt-bridge binary handles actual MQTT I/O; annotations live here because
// only the api binary runs the Lucyd docs server.

/// ESP32 → Server: button press/release events from GPIO inputs.
#[lucy_mqtt(
    topic       = "pinball/{device_id}/input/button",
    tags        = "mqtt, input",
    request     = ButtonInput,
    description = "Button press or release from the ESP32 GPIO inputs (L1, R1, L2, R2, Start, under_plunger, top, middle, bottom)",
)]
pub async fn mqtt_input_button() {}

/// ESP32 → Server: plunger press/release.
#[lucy_mqtt(
    topic       = "pinball/{device_id}/input/plunger",
    tags        = "mqtt, input",
    request     = PlungerInput,
    description = "Plunger press or release (state: 1=pressed, 0=released)",
)]
pub async fn mqtt_input_plunger() {}

/// ESP32 → Server: gyroscope / tilt detection at ~20 Hz.
#[lucy_mqtt(
    topic       = "pinball/{device_id}/input/gyro",
    tags        = "mqtt, input",
    request     = GyroInput,
    description = "Raw accelerometer values and tilt flag from the ESP32 gyroscope (~20 Hz)",
)]
pub async fn mqtt_input_gyro() {}

/// ESP32 → Server: device telemetry every 10 s.
#[lucy_mqtt(
    topic       = "pinball/{device_id}/telemetry",
    tags        = "mqtt, telemetry",
    request     = Telemetry,
    description = "Device metrics published every 10 s (wifi_rssi, uptime, loop frequency, free heap, MQTT reconnects)",
)]
pub async fn mqtt_telemetry() {}

/// ESP32 → Server: lifecycle events (boot, ack, alert, error, OTA).
#[lucy_mqtt(
    topic       = "pinball/{device_id}/events",
    tags        = "mqtt, events",
    request     = DeviceEvent,
    description = "ESP32 lifecycle event (boot, ack, alert, error, ota_start, ota_done)",
)]
pub async fn mqtt_events() {}

/// ESP32 → Server: device status with retain flag.
#[lucy_mqtt(
    topic       = "pinball/{device_id}/status",
    tags        = "mqtt, status",
    request     = DeviceStatus,
    description = "Device online status (retained). Also used as LWT with {online: false} for disconnect detection",
)]
pub async fn mqtt_status() {}

/// Server → ESP32: active ball collisions list.
#[lucy_mqtt(
    topic       = "pinball/{device_id}/ball/hit",
    tags        = "mqtt, output",
    response    = BallHit,
    description = "Active collision list published each time a collision starts or ends. Empty hits array stops all vibration",
)]
pub async fn mqtt_ball_hit() {}

/// Server → ESP32: game state snapshot with retain flag.
#[lucy_mqtt(
    topic       = "pinball/{device_id}/game/state",
    tags        = "mqtt, output",
    response    = GameState,
    description = "Current game state snapshot (retained). Published on every game phase transition",
)]
pub async fn mqtt_game_state() {}

/// Server → ESP32: remote command (vibrate, reboot, OTA, set_config).
#[lucy_mqtt(
    topic       = "pinball/{device_id}/cmd",
    tags        = "mqtt, output",
    response    = Command,
    description = "Remote command sent to the ESP32 (vibrate, reboot, ota, set_config)",
)]
pub async fn mqtt_cmd() {}
