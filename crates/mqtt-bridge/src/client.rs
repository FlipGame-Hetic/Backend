use rumqttc::{AsyncClient, EventLoop, MqttOptions, QoS};
use shared::dto::Topic;
use tracing::info;

use crate::config::BridgeConfig;
use crate::errors::Result;

pub struct MqttClient {
    client: AsyncClient,
    event_loop: EventLoop,
}

impl MqttClient {
    pub fn new(config: &BridgeConfig) -> Self {
        let mut opts =
            MqttOptions::new(&config.mqtt_client_id, &config.mqtt_host, config.mqtt_port);
        opts.set_keep_alive(std::time::Duration::from_secs(
            config.mqtt_keep_alive_secs as u64,
        ));
        opts.set_clean_session(true);

        let (client, event_loop) = AsyncClient::new(opts, config.mqtt_channel_capacity);

        info!(
            host = %config.mqtt_host,
            port = config.mqtt_port,
            client_id = %config.mqtt_client_id,
            "mqtt client created"
        );

        Self { client, event_loop }
    }

    /// Subscribe to all pinball device topics (`pinball/+/#`).
    pub async fn subscribe_all(&self) -> Result<()> {
        let filter = Topic::subscribe_all();
        self.client.subscribe(filter, QoS::AtLeastOnce).await?;
        info!(filter, "subscribed to all device topics");
        Ok(())
    }

    /// Split into the underlying `AsyncClient` (for publishing) and `EventLoop` (for polling).
    /// The event loop must be polled continuously for the client to function.
    pub fn split(self) -> (AsyncClient, EventLoop) {
        (self.client, self.event_loop)
    }
}
