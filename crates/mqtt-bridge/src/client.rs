//! Thin wrapper around `rumqttc`'s [`AsyncClient`] and [`EventLoop`].
//!
//! [`MqttClient`] bundles construction and initial subscription into a single
//! convenient type, then exposes [`split`][MqttClient::split] so the bridge
//! can hand the event loop to a dedicated polling task while keeping the client
//! handle for publishing.
use rumqttc::{AsyncClient, EventLoop, MqttOptions, QoS};
use shared::dto::Topic;
use tracing::info;

use crate::config::BridgeConfig;
use crate::errors::Result;

/// A configured MQTT async client paired with its event loop.
///
/// The two halves are tightly coupled: the [`AsyncClient`] queues outgoing
/// packets that the [`EventLoop`] drives over the TCP connection.  Call
/// [`split`][Self::split] to separate them so they can be used across
/// concurrent tasks.
pub struct MqttClient {
    client: AsyncClient,
    event_loop: EventLoop,
}

impl MqttClient {
    /// Construct and configure a new MQTT client from [`BridgeConfig`].
    ///
    /// `clean_session = true` tells the broker to discard any QoS-1 messages
    /// queued while the bridge was offline, preventing a replay flood of stale
    /// device events on reconnect.
    pub fn new(config: &BridgeConfig) -> Self {
        let mut opts =
            MqttOptions::new(&config.mqtt_client_id, &config.mqtt_host, config.mqtt_port);
        opts.set_keep_alive(std::time::Duration::from_secs(
            config.mqtt_keep_alive_secs as u64,
        ));
        // Discard broker-side queues between sessions to avoid replaying stale state
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
    ///
    /// The wildcard filter string is centralised in [`Topic::subscribe_all`] so
    /// the topic scheme has a single authoritative definition.
    pub async fn subscribe_all(&self) -> Result<()> {
        let filter = Topic::subscribe_all();
        self.client.subscribe(filter, QoS::AtLeastOnce).await?;
        info!(filter, "subscribed to all device topics");
        Ok(())
    }

    /// Split into the underlying [`AsyncClient`] (for publishing) and
    /// [`EventLoop`] (for polling).
    ///
    /// The event loop **must** be polled continuously — stalling it blocks the
    /// client's outbound queue and prevents ACKs from being processed.
    pub fn split(self) -> (AsyncClient, EventLoop) {
        (self.client, self.event_loop)
    }
}
