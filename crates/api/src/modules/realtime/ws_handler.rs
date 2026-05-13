use axum::extract::ws::{Message, WebSocket};
use axum::extract::{State, WebSocketUpgrade};
use axum::response::IntoResponse;
use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use lucyd::lucy_ws;
use shared::events::WsMessage;
use tokio::sync::broadcast;
use tracing::{debug, error, info, warn};

use crate::modules::scores::dto::SaveScoreRequest;
use crate::modules::scores::service as score_service;
use crate::state::AppState;

use super::bridge_sync::sync_game_state_to_bridge;

/// Axum handler: upgrade HTTP to WebSocket for a bridge connection.
#[lucy_ws(
    path = "/ws/bridge",
    tags = "realtime",
    description = "MQTT bridge WebSocket, bidirectional relay between the backend and the mqtt-bridge process"
)]
pub async fn ws_bridge(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    info!("bridge websocket upgrade requested");
    ws.on_upgrade(|socket| handle_bridge(socket, state))
}

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

async fn read_loop(mut stream: futures_util::stream::SplitStream<WebSocket>, state: AppState) {
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

        match ws_msg {
            WsMessage::Inbound {
                ref device_id,
                ref payload,
            } => {
                debug!(device_id = %device_id, payload = ?payload, "received inbound from bridge");

                // Update the active device id so screen handlers know where to sync
                {
                    let mut id_guard = state.active_device_id.write().await;
                    *id_guard = Some(device_id.clone());
                }

                process_inbound(&state, device_id.clone(), payload).await;
            }
            WsMessage::Outbound { .. } => {
                warn!("received outbound message from bridge (unexpected direction), ignoring");
            }
        }
    }
}

async fn process_inbound(
    state: &AppState,
    device_id: String,
    payload: &shared::events::InboundMessage,
) {
    // Lock order: engine FIRST, session SECOND [§ 4.4]
    let mut engine_guard = state.game_engine.lock().await;
    let mut session_guard = state.active_session.lock().await;

    let Some(engine) = engine_guard.as_mut() else {
        return;
    };

    let envelopes = engine.handle_inbound(payload);

    // Track boss defeats and detect game over
    for env in &envelopes {
        if env.event_type == "BossDefeated"
            && let Some(session) = session_guard.as_mut()
        {
            session.boss_reached = session.boss_reached.saturating_add(1);
        }
    }

    let game_over = envelopes.iter().any(|e| e.event_type == "GameOver");
    let score_snapshot = engine.state.score;
    let state_snapshot = engine.state.clone();

    let session_snapshot = if game_over {
        *engine_guard = None;
        session_guard.take()
    } else {
        None
    };

    // Unlock before any await [§ 4.4]
    drop(session_guard);
    drop(engine_guard);

    for env in envelopes {
        let _ = state.screen_router.dispatch(env).await;
    }

    sync_game_state_to_bridge(&state_snapshot, &state.hub, &device_id);

    if game_over && let Some(session) = session_snapshot {
        let req = SaveScoreRequest {
            player_id: session.player_id,
            character_id: session.character_id,
            score: score_snapshot,
            boss_reached: session.boss_reached,
        };
        if let Err(e) = score_service::save_score(&state.db_pool, req).await {
            error!(error = %e, "failed to persist score after game over");
        }
    }
}

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
