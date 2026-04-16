use axum::extract::ws::{Message, WebSocket};
use axum::extract::{State, WebSocketUpgrade};
use axum::response::IntoResponse;
use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use lucy::lucy_ws;
use shared::events::WsMessage;
use tokio::sync::broadcast;
use tracing::{debug, error, info, warn};

use crate::state::AppState;

/// Axum handler: upgrade HTTP to WebSocket for a bridge connection.
#[lucy_ws(path = "/ws/bridge", tags = "realtime", description = "MQTT bridge WebSocket, bidirectional relay between the backend and the mqtt-bridge process")]
pub async fn ws_bridge(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    info!("bridge websocket upgrade requested");
    ws.on_upgrade(|socket| handle_bridge(socket, state))
}

/// Manages a single bridge WebSocket connection lifetime
///
/// Spawns two concurrent tasks:
/// - **read**: receives `WsMessage::Inbound` from the bridge, logs them,
///   and can forward to game logic in the future.
/// - **write**: listens on the hub broadcast and forwards `WsMessage::Outbound`
///   to the bridge so it can publish them over MQTT.
async fn handle_bridge(socket: WebSocket, state: AppState) {
    info!("bridge websocket connected");

    let (sink, stream) = socket.split();
    let hub_rx = state.hub.subscribe();

    let write_handle = tokio::spawn(write_loop(sink, hub_rx));
    let read_handle = tokio::spawn(read_loop(stream, state));

    tokio::select! {
        res = read_handle => {
            match res {
                Ok(()) => info!("bridge read loop ended"),
                Err(e) => error!(error = %e, "bridge read loop panicked"),
            }
        }
        res = write_handle => {
            match res {
                Ok(()) => info!("bridge write loop ended"),
                Err(e) => error!(error = %e, "bridge write loop panicked"),
            }
        }
    }

    info!("bridge websocket disconnected");
}

/// Read frames from the bridge, deserialize as `WsMessage`, and process.
async fn read_loop(mut stream: futures_util::stream::SplitStream<WebSocket>, _state: AppState) {
    while let Some(frame) = stream.next().await {
        let msg = match frame {
            Ok(m) => m,
            Err(e) => {
                warn!(error = %e, "bridge ws read error");
                break;
            }
        };

        let text = match msg {
            Message::Text(t) => t,
            Message::Close(_) => {
                info!("bridge sent close frame");
                break;
            }
            Message::Ping(_) | Message::Pong(_) => continue,
            Message::Binary(_) => {
                debug!("ignoring binary frame from bridge");
                continue;
            }
        };

        let ws_msg: WsMessage = match serde_json::from_str(&text) {
            Ok(m) => m,
            Err(e) => {
                warn!(error = %e, "invalid JSON from bridge");
                continue;
            }
        };

        match &ws_msg {
            WsMessage::Inbound { device_id, payload } => {
                debug!(
                    device_id = %device_id,
                    payload = ?payload,
                    "received inbound from bridge"
                );
                // TODO: forward to game logic / event processing pipeline
            }
            WsMessage::Outbound { .. } => {
                warn!("received outbound message from bridge (unexpected direction), ignoring");
            }
        }
    }
}

/// Subscribe to hub broadcast and forward outbound messages to the bridge sink.
async fn write_loop(
    mut sink: SplitSink<WebSocket, Message>,
    mut hub_rx: broadcast::Receiver<WsMessage>,
) {
    loop {
        let msg = match hub_rx.recv().await {
            Ok(m) => m,
            Err(broadcast::error::RecvError::Lagged(n)) => {
                warn!(skipped = n, "bridge ws write lagged, some messages dropped");
                continue;
            }
            Err(broadcast::error::RecvError::Closed) => {
                info!("hub broadcast closed, stopping write loop");
                break;
            }
        };

        let json = match serde_json::to_string(&msg) {
            Ok(j) => j,
            Err(e) => {
                error!(error = %e, "failed to serialize outbound message");
                continue;
            }
        };

        if let Err(e) = sink.send(Message::Text(json.into())).await {
            warn!(error = %e, "failed to send to bridge ws, closing write loop");
            break;
        }
    }
}
