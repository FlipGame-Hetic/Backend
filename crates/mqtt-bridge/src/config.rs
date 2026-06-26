//! Runtime configuration for the MQTT bridge, sourced entirely from
//! environment variables.
//!
//! Every variable has a hard-coded default so the bridge can start without
//! any explicit configuration (handy for local dev and Docker Compose setups
//! that omit optional vars).
use crate::errors::{BridgeError, Result};

/// All tuneable parameters for the MQTT bridge, resolved once at startup.
#[derive(Debug, Clone)]
pub struct BridgeConfig {
    /// Hostname or IP of the MQTT broker (e.g. `mosquitto` inside Docker).
    pub mqtt_host: String,
    /// TCP port the broker listens on; standard unencrypted MQTT uses 1883.
    pub mqtt_port: u16,
    /// Unique client identifier sent to the broker during the CONNECT handshake.
    pub mqtt_client_id: String,
    /// Keep-alive interval in seconds.  The broker disconnects a client that
    /// sends no packet within 1.5× this value.
    pub mqtt_keep_alive_secs: u16,
    /// Depth of the internal `rumqttc` channel that buffers outgoing publishes.
    /// Increase this if you observe `SendError` drops under high publish bursts.
    pub mqtt_channel_capacity: usize,
    /// WebSocket URL of the central API's bridge endpoint.
    pub backend_ws_url: String,
    /// Seconds to wait between reconnect attempts after the relay fails.
    pub reconnect_delay_secs: u64,
}

impl BridgeConfig {
    /// Build a [`BridgeConfig`] by reading environment variables.
    ///
    /// Returns [`BridgeError::Config`] when a variable is present but cannot
    /// be parsed into the expected numeric type.  Missing variables silently
    /// fall back to sensible defaults.
    pub fn from_env() -> Result<Self> {
        let mqtt_host = std::env::var("MQTT_HOST").unwrap_or_else(|_| "mosquitto".to_owned());

        let mqtt_port = std::env::var("MQTT_PORT")
            .unwrap_or_else(|_| "1883".to_owned())
            .parse::<u16>()
            .map_err(|_| BridgeError::Config("MQTT_PORT must be a valid u16".to_owned()))?;

        let mqtt_client_id =
            std::env::var("MQTT_CLIENT_ID").unwrap_or_else(|_| "mqtt-bridge".to_owned());

        let mqtt_keep_alive_secs = std::env::var("MQTT_KEEP_ALIVE_SECS")
            .unwrap_or_else(|_| "30".to_owned())
            .parse::<u16>()
            .map_err(|_| {
                BridgeError::Config("MQTT_KEEP_ALIVE_SECS must be a valid u16".to_owned())
            })?;

        let mqtt_channel_capacity = std::env::var("MQTT_CHANNEL_CAPACITY")
            .unwrap_or_else(|_| "128".to_owned())
            .parse::<usize>()
            .map_err(|_| {
                BridgeError::Config("MQTT_CHANNEL_CAPACITY must be a valid usize".to_owned())
            })?;

        let backend_ws_url = std::env::var("BACKEND_WS_URL")
            .unwrap_or_else(|_| "ws://api:8080/ws/bridge".to_owned());

        let reconnect_delay_secs = std::env::var("RECONNECT_DELAY_SECS")
            .unwrap_or_else(|_| "3".to_owned())
            .parse::<u64>()
            .map_err(|_| {
                BridgeError::Config("RECONNECT_DELAY_SECS must be a valid u64".to_owned())
            })?;

        Ok(Self {
            mqtt_host,
            mqtt_port,
            mqtt_client_id,
            mqtt_keep_alive_secs,
            mqtt_channel_capacity,
            backend_ws_url,
            reconnect_delay_secs,
        })
    }
}
