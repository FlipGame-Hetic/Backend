use crate::errors::{BridgeError, Result};

#[derive(Debug, Clone)]
pub struct BridgeConfig {
    pub mqtt_host: String,
    pub mqtt_port: u16,
    pub mqtt_client_id: String,
    pub mqtt_keep_alive_secs: u16,
    pub mqtt_channel_capacity: usize,
    pub backend_ws_url: String,
    pub reconnect_delay_secs: u64,
}

impl BridgeConfig {
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