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

/// Channel capacity for internal message passing between tasks.
const INTERNAL_CHANNEL_SIZE: usize = 256;

/// Bidirectional relay between MQTT (local broker) and WebSocket (central API).
///
/// Runs three concurrent tasks:
/// 1. **mqtt_loop**: polls the MQTT event loop, parses inbound publishes,
///    and forwards them as `WsMessage::Inbound` to the WS sender.
/// 2. **ws_read_loop**: reads `WsMessage::Outbound` from the API and
///    forwards them as MQTT publishes to the local broker.
/// 3. **ws_write_loop**: drains the internal channel and writes JSON frames
///    to the WebSocket sink.
pub struct Bridge {
    config: BridgeConfig,
}

impl Bridge {
    pub fn new(config: BridgeConfig) -> Self {
        Self { config }
    }

    /// Run the bridge forever, reconnecting on failure.
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
    async fn run_once(&self) -> Result<()> {
        // MQTT client
        let mqtt = MqttClient::new(&self.config);
        mqtt.subscribe_all().await?;
        let (mqtt_client, mqtt_event_loop) = mqtt.split();

        // WebSocket client
        info!(url = %self.config.backend_ws_url, "connecting to central API");

        let (ws_stream, _response) =
            tokio_tungstenite::connect_async(&self.config.backend_ws_url).await?;

        info!("websocket connected to central API");

        let (ws_sink, ws_source) = ws_stream.split();

        // Internal channel: mqtt_loop → ws_write_loop
        let (ws_tx, ws_rx) = mpsc::channel::<WsMessage>(INTERNAL_CHANNEL_SIZE);

        // Spawn all three tasks, abort on first failure
        let mqtt_handle = tokio::spawn(mqtt_inbound_loop(mqtt_event_loop, ws_tx));

        let ws_write_handle = tokio::spawn(ws_write_loop(ws_rx, ws_sink));

        let ws_read_handle = tokio::spawn(ws_outbound_loop(ws_source, mqtt_client));

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

// Task 1: MQTT event loop => parse => channel

async fn mqtt_inbound_loop(
    mut event_loop: EventLoop,
    ws_tx: mpsc::Sender<WsMessage>,
) -> Result<()> {
    info!("mqtt inbound loop started");

    loop {
        let event = match event_loop.poll().await {
            Ok(event) => event,
            Err(e) => {
                error!(error = %e, "mqtt poll error");
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                continue;
            }
        };

        match event {
            Event::Incoming(Packet::Publish(publish)) => {
                let topic = &publish.topic;
                let payload = &publish.payload;

                match handler::handle_publish(topic, payload) {
                    Ok(handled) => {
                        let ws_msg = WsMessage::Inbound {
                            device_id: handled.device_id,
                            payload: handled.message,
                        };

                        if ws_tx.send(ws_msg).await.is_err() {
                            warn!("WS channel closed, stopping mqtt loop");
                            return Ok(());
                        }
                    }
                    Err(e) => {
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
            Event::Incoming(Packet::PingResp) | Event::Outgoing(_) => {}
            other => {
                debug!(event = ?other, "Unhandled MQTT event");
            }
        }
    }
}

// Task 2: Channel => WebSocket sink

async fn ws_write_loop<S>(mut rx: mpsc::Receiver<WsMessage>, mut sink: S) -> Result<()>
where
    S: SinkExt<WsRawMessage, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
{
    info!("WS write loop started");

    while let Some(msg) = rx.recv().await {
        let json = serde_json::to_string(&msg)?;

        debug!(len = json.len(), "Sending to central API");

        sink.send(WsRawMessage::Text(json.into())).await?;
    }

    info!("WS write loop ended (channel closed)");
    Ok(())
}

// Task 3: WebSocket source => MQTT publish

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
            WsRawMessage::Ping(_) => continue,
            WsRawMessage::Pong(_) => continue,
            WsRawMessage::Close(_) => {
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
            WsMessage::Outbound { device_id, payload } => {
                if let Err(e) = publish_outbound(&mqtt_client, &device_id, &payload).await {
                    warn!(device_id, error = %e, "Failed to publish outbound to MQTT");
                }
            }
            WsMessage::Inbound { .. } => {
                warn!("Received inbound message from API (unexpected direction), ignoring");
            }
        }
    }

    info!("WS outbound loop ended (stream closed)");
    Ok(())
}

// Helpers

async fn publish_outbound(
    client: &AsyncClient,
    device_id: &str,
    payload: &OutboundMessage,
) -> Result<()> {
    let (subtopic, retain) = match payload {
        OutboundMessage::BallHit(_) => (Subtopic::BallHit, false),
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

    debug!(
        topic = %topic_str,
        device_id,
        retain,
        "Published outbound to mqtt"
    );

    Ok(())
}
