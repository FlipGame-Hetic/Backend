//! Core relay logic: three concurrent tasks wired together to bridge MQTT and
//! WebSocket.
//!
//! The data flow is:
//! ```text
//! MQTT broker ──► mqtt_inbound_loop ──► [mpsc channel] ──► ws_write_loop ──► central API
//! central API ──► ws_outbound_loop  ──► MQTT broker
//! ```
use futures_util::{SinkExt, StreamExt};
use rumqttc::{AsyncClient, Event, EventLoop, Packet, QoS};
use shared::dto::{Subtopic, Topic};
use shared::events::{OutboundMessage, WsMessage};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message as WsRawMessage;
use tracing::{debug, error, info, warn};

use crate::client::MqttClient;
use crate::config::BridgeConfig;
use crate::errors::Result;
use crate::handler;

/// Depth of the mpsc channel that connects the MQTT inbound task to the WS
/// write task.  256 provides headroom for short traffic bursts without letting
/// memory grow unboundedly.
const INTERNAL_CHANNEL_SIZE: usize = 256;

/// Bidirectional relay between MQTT (local broker) and WebSocket (central API).
///
/// Runs three concurrent tasks:
/// 1. **`mqtt_inbound_loop`**: polls the MQTT event loop, parses inbound
///    `Publish` packets, and forwards them as `WsMessage::Inbound` to the WS
///    write task.
/// 2. **`ws_outbound_loop`**: reads `WsMessage::Outbound` frames from the API
///    and publishes them to the local MQTT broker.
/// 3. **`ws_write_loop`**: drains the internal channel and serialises each
///    message as a JSON text frame to the WebSocket sink.
pub struct Bridge {
    config: BridgeConfig,
}

impl Bridge {
    pub fn new(config: BridgeConfig) -> Self {
        Self { config }
    }

    /// Run the bridge forever, reconnecting on failure.
    ///
    /// Returns `!` — this function never resolves normally.  Each loop
    /// iteration calls [`run_once`][Self::run_once], which exits when any of
    /// the three relay tasks terminates.  After a configurable back-off delay
    /// the whole connection is re-established from scratch.
    pub async fn run(&self) -> ! {
        loop {
            info!("starting bridge relay");

            if let Err(e) = self.run_once().await {
                error!(error = %e, "bridge relay failed");
            }

            let delay = self.config.reconnect_delay_secs;
            warn!(delay_secs = delay, "reconnecting in...");
            tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
        }
    }

    /// Single connection lifecycle: connect both sides, relay until one drops.
    ///
    /// Spawns the three relay tasks and uses `tokio::select!` so that the
    /// **first** task to exit — cleanly or with an error — causes this function
    /// to return.  The remaining task handles are dropped, which aborts them.
    async fn run_once(&self) -> Result<()> {
        let mqtt = MqttClient::new(&self.config);
        mqtt.subscribe_all().await?;
        let (mqtt_client, mqtt_event_loop) = mqtt.split();

        info!(url = %self.config.backend_ws_url, "connecting to central API");

        let (ws_stream, _response) =
            tokio_tungstenite::connect_async(&self.config.backend_ws_url).await?;

        info!("websocket connected to central API");

        let (ws_sink, ws_source) = ws_stream.split();

        // Bounded channel so a slow WS sink back-pressures the MQTT task rather
        // than accumulating messages in memory during a traffic burst.
        let (ws_tx, ws_rx) = mpsc::channel::<WsMessage>(INTERNAL_CHANNEL_SIZE);

        let mqtt_handle = tokio::spawn(mqtt_inbound_loop(mqtt_event_loop, ws_tx));
        let ws_write_handle = tokio::spawn(ws_write_loop(ws_rx, ws_sink));
        let ws_read_handle = tokio::spawn(ws_outbound_loop(ws_source, mqtt_client));

        // Wait for whichever task finishes first; dropping the remaining handles
        // cancels those tasks so we don't leave orphaned futures running.
        tokio::select! {
            res = mqtt_handle => {
                let msg = "mqtt loop exited";
                match res {
                    Ok(Ok(())) => info!(msg),
                    Ok(Err(e)) => error!(error = %e, "{msg}"),
                    Err(e) => error!(error = %e, "{msg} (join error)"),
                }
            }
            res = ws_write_handle => {
                let msg = "ws write loop exited";
                match res {
                    Ok(Ok(())) => info!(msg),
                    Ok(Err(e)) => error!(error = %e, "{msg}"),
                    Err(e) => error!(error = %e, "{msg} (join error)"),
                }
            }
            res = ws_read_handle => {
                let msg = "ws read loop exited";
                match res {
                    Ok(Ok(())) => info!(msg),
                    Ok(Err(e)) => error!(error = %e, "{msg}"),
                    Err(e) => error!(error = %e, "{msg} (join error)"),
                }
            }
        }

        Ok(())
    }
}

/// Poll the MQTT event loop, parse `Publish` packets, and forward them to the
/// WebSocket write task via the internal channel.
async fn mqtt_inbound_loop(
    mut event_loop: EventLoop,
    ws_tx: mpsc::Sender<WsMessage>,
) -> Result<()> {
    info!("mqtt inbound loop started");

    loop {
        let event = match event_loop.poll().await {
            Ok(event) => event,
            Err(e) => {
                // Non-fatal poll error (e.g. transient TCP hiccup); sleep briefly
                // to avoid a tight error loop that would burn CPU.
                error!(error = %e, "mqtt poll error");
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                continue;
            }
        };

        match event {
            Event::Incoming(Packet::Publish(publish)) => {
                let topic = &publish.topic;
                let payload = &publish.payload;

                info!(
                    topic = %topic,
                    payload_len = payload.len(),
                    payload_utf8 = %String::from_utf8_lossy(payload),
                    "[MQTT ←] received publish from broker"
                );

                match handler::handle_publish(topic, payload) {
                    Ok(handled) => {
                        let ws_msg = WsMessage::Inbound {
                            device_id: handled.device_id,
                            payload: handled.message,
                        };

                        // A closed channel means the WS write task has already
                        // exited; stop cleanly so the bridge can reconnect.
                        if ws_tx.send(ws_msg).await.is_err() {
                            warn!("WS channel closed, stopping mqtt loop");
                            return Ok(());
                        }
                    }
                    Err(e) => {
                        // Unrecognised or outbound-only topic — skip without crashing.
                        debug!(topic, error = %e, "Skipping unhandled message");
                    }
                }
            }
            Event::Incoming(Packet::ConnAck(_)) => {
                info!("MQTT connected to broker");
            }
            Event::Incoming(Packet::SubAck(_)) => {
                debug!("MQTT subscription acknowledged");
            }
            // PingResp keeps the TCP session alive; outgoing packets are our own
            // queued publishes.  Neither requires any action here.
            Event::Incoming(Packet::PingResp) | Event::Outgoing(_) => {}
            other => {
                debug!(event = ?other, "Unhandled MQTT event");
            }
        }
    }
}

/// Drain the internal channel and write each message as a JSON text frame to
/// the WebSocket sink.
async fn ws_write_loop<S>(mut rx: mpsc::Receiver<WsMessage>, mut sink: S) -> Result<()>
where
    S: SinkExt<WsRawMessage, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
{
    info!("WS write loop started");

    while let Some(msg) = rx.recv().await {
        let json = serde_json::to_string(&msg)?;

        info!(len = json.len(), payload = %json, "[WS →] sending to API");

        sink.send(WsRawMessage::Text(json.into())).await?;
    }

    // All senders were dropped (mqtt_inbound_loop exited); signal clean shutdown.
    info!("WS write loop ended (channel closed)");
    Ok(())
}

/// Read JSON frames from the WebSocket and publish `Outbound` commands to the
/// MQTT broker.
async fn ws_outbound_loop<S>(mut source: S, mqtt_client: AsyncClient) -> Result<()>
where
    S: StreamExt<Item = std::result::Result<WsRawMessage, tokio_tungstenite::tungstenite::Error>>
        + Unpin,
{
    info!("WS outbound loop started");

    while let Some(frame) = source.next().await {
        let frame = frame?;

        let text = match &frame {
            WsRawMessage::Text(t) => t.as_ref(),
            // Ping/Pong are handled transparently by tungstenite; no action needed.
            WsRawMessage::Ping(_) => continue,
            WsRawMessage::Pong(_) => continue,
            WsRawMessage::Close(_) => {
                // Graceful close from the server; return cleanly so the bridge
                // reconnects instead of spinning on a dead stream.
                info!("Central API closed websocket");
                return Ok(());
            }
            other => {
                debug!(?other, "Ignoring non-text ws frame");
                continue;
            }
        };

        let ws_msg: WsMessage = match serde_json::from_str(text) {
            Ok(m) => m,
            Err(e) => {
                warn!(error = %e, "Invalid JSON from central API");
                continue;
            }
        };

        match ws_msg {
            WsMessage::Outbound {
                device_id,
                ref payload,
            } => {
                info!(device_id = %device_id, payload = ?payload, "[WS ←] received outbound from API");
                if let Err(e) = publish_outbound(&mqtt_client, &device_id, payload).await {
                    warn!(device_id, error = %e, "Failed to publish outbound to MQTT");
                }
            }
            // The API should only ever send Outbound messages to this endpoint.
            WsMessage::Inbound { .. } => {
                warn!("Received inbound message from API (unexpected direction), ignoring");
            }
        }
    }

    info!("WS outbound loop ended (stream closed)");
    Ok(())
}

/// Publish an outbound command from the central API to the per-device MQTT topic.
///
/// `retain` is `true` only for `GameState` so that a freshly connected ESP32
/// immediately receives the current game state without waiting for the next
/// server-side update cycle.
async fn publish_outbound(
    client: &AsyncClient,
    device_id: &str,
    payload: &OutboundMessage,
) -> Result<()> {
    let (subtopic, retain) = match payload {
        OutboundMessage::BallHit(_) => (Subtopic::BallHit, false),
        // Retained so a rebooting device gets the current state as soon as it subscribes
        OutboundMessage::GameState(_) => (Subtopic::GameState, true),
        OutboundMessage::Command(_) => (Subtopic::Cmd, false),
    };

    let topic = Topic {
        device_id: device_id.to_owned(),
        subtopic,
    };

    let topic_str = topic.to_mqtt_topic();
    let bytes = serde_json::to_vec(payload)?;

    client
        .publish(&topic_str, QoS::AtLeastOnce, retain, bytes)
        .await?;

    info!(
        topic = %topic_str,
        device_id,
        retain,
        "[MQTT →] published to broker"
    );

    Ok(())
}
