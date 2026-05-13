use axum::extract::ws::{Message, WebSocket};
use axum::extract::{Path, Query, State, WebSocketUpgrade};
use axum::response::IntoResponse;
use futures_util::{SinkExt, StreamExt};
use game_logic::GameEngine;
use lucyd::lucy_ws;
use serde::Deserialize;
use shared::screen::{ScreenEnvelope, ScreenId};
use tracing::{debug, error, info, warn};

use crate::errors::ApiError;
use crate::modules::realtime::bridge_sync::sync_game_state_to_bridge;
use crate::modules::scores::dto::SaveScoreRequest;
use crate::modules::scores::service as score_service;
use crate::state::{AppState, GameSession};

use super::auth;

#[derive(Debug, Deserialize)]
pub struct TokenQuery {
    pub token: String,
}

#[lucy_ws(
    path        = "/ws/screen/{screen_id}",
    tags        = "screens, realtime",
    request     = ScreenEnvelope,
    description = "Per-screen WebSocket, authenticated by JWT, relays ScreenEnvelope frames",
)]
pub async fn ws_screen(
    ws: WebSocketUpgrade,
    Path(screen_id_raw): Path<String>,
    Query(query): Query<TokenQuery>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let screen_id: ScreenId = screen_id_raw
        .parse()
        .map_err(|_| ApiError::BadRequest(format!("unknown screen id: '{screen_id_raw}'")))?;

    auth::verify_and_match(&query.token, &state.jwt_secret, screen_id).map_err(|e| {
        warn!(screen = %screen_id, error = %e, "screen auth failed");
        ApiError::BadRequest(format!("authentication failed: {e}"))
    })?;

    info!(screen = %screen_id, "screen websocket upgrade accepted");

    Ok(ws.on_upgrade(move |socket| handle_screen(socket, screen_id, state)))
}

async fn handle_screen(socket: WebSocket, screen_id: ScreenId, state: AppState) {
    info!(screen = %screen_id, "screen websocket connected");

    let handle = match state.screen_registry.register(screen_id).await {
        Ok(h) => h,
        Err(e) => {
            error!(screen = %screen_id, error = %e, "failed to register screen");
            return;
        }
    };

    let (rx, _guard) = handle.into_parts();

    let (sink, stream) = socket.split();

    let write_handle = tokio::spawn(write_loop(screen_id, rx, sink));
    let read_handle = tokio::spawn(read_loop(screen_id, stream, state));

    tokio::select! {
        res = read_handle => {
            match res {
                Ok(()) => info!(screen = %screen_id, "screen read loop ended"),
                Err(e) => error!(screen = %screen_id, error = %e, "screen read loop panicked"),
            }
        }
        res = write_handle => {
            match res {
                Ok(()) => info!(screen = %screen_id, "screen write loop ended"),
                Err(e) => error!(screen = %screen_id, error = %e, "screen write loop panicked"),
            }
        }
    }

    info!(screen = %screen_id, "screen websocket disconnected");
}

async fn read_loop(
    screen_id: ScreenId,
    mut stream: futures_util::stream::SplitStream<WebSocket>,
    state: AppState,
) {
    while let Some(frame) = stream.next().await {
        let msg = match frame {
            Ok(m) => m,
            Err(e) => {
                warn!(screen = %screen_id, error = %e, "ws read error");
                break;
            }
        };

        let text = match msg {
            Message::Text(t) => t,
            Message::Close(_) => {
                info!(screen = %screen_id, "screen sent close frame");
                break;
            }
            Message::Ping(_) | Message::Pong(_) => continue,
            Message::Binary(_) => {
                debug!(screen = %screen_id, "ignoring binary frame");
                continue;
            }
        };

        let envelope: ScreenEnvelope = match serde_json::from_str(&text) {
            Ok(e) => e,
            Err(e) => {
                warn!(screen = %screen_id, error = %e, "invalid JSON from screen");
                continue;
            }
        };

        if envelope.from != screen_id {
            warn!(
                screen = %screen_id,
                claimed_from = %envelope.from,
                "screen tried to spoof 'from' field, ignoring"
            );
            continue;
        }

        let result = state.screen_router.dispatch(envelope.clone()).await;
        debug!(
            screen = %screen_id,
            delivered = result.delivered,
            missed = ?result.missed,
            intercepted = result.intercepted,
            "dispatch result"
        );

        process_screen_event(&state, &envelope).await;
    }
}

async fn process_screen_event(state: &AppState, envelope: &ScreenEnvelope) {
    if envelope.event_type == "StartGame" {
        handle_start_game(state, envelope).await;
    } else {
        handle_game_event(state, envelope).await;
    }
}

async fn handle_start_game(state: &AppState, envelope: &ScreenEnvelope) {
    let player_id = envelope
        .payload
        .get("player_id")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_owned();
    let character_id = envelope
        .payload
        .get("character_id")
        .and_then(|v| v.as_u64())
        .unwrap_or(1) as u8;

    // Lock order: engine FIRST, session SECOND [§ 4.4]
    let mut engine_guard = state.game_engine.lock().await;
    let mut session_guard = state.active_session.lock().await;

    // Policy: silently ignore if a game is already in progress [§ 4.2]
    if session_guard.is_some() {
        warn!("StartGame ignored — game already in progress");
        drop(session_guard);
        drop(engine_guard);
        return;
    }

    let mut engine = GameEngine::new(character_id);
    let envelopes = engine.process(game_logic::GameEvent::StartGame {
        player_id: player_id.clone(),
    });

    let state_snapshot = engine.state.clone();

    *engine_guard = Some(engine);
    *session_guard = Some(GameSession {
        player_id,
        character_id,
        boss_reached: 0,
    });

    // Unlock before any await [§ 4.4]
    drop(session_guard);
    drop(engine_guard);

    let device_id = state.active_device_id.read().await.clone();
    if device_id.is_none() {
        warn!("no bridge connected — ESP32 sync skipped");
    }

    for env in envelopes {
        let _ = state.screen_router.dispatch(env).await;
    }

    if let Some(id) = device_id {
        sync_game_state_to_bridge(&state_snapshot, &state.hub, &id);
    }
}

async fn handle_game_event(state: &AppState, envelope: &ScreenEnvelope) {
    // Lock order: engine FIRST, session SECOND [§ 4.4]
    let mut engine_guard = state.game_engine.lock().await;
    let mut session_guard = state.active_session.lock().await;

    let Some(engine) = engine_guard.as_mut() else {
        drop(session_guard);
        drop(engine_guard);
        return;
    };

    let envelopes = engine.handle_screen_event(envelope);

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

    let device_id = state.active_device_id.read().await.clone();

    for env in envelopes {
        let _ = state.screen_router.dispatch(env).await;
    }

    if let Some(id) = device_id {
        sync_game_state_to_bridge(&state_snapshot, &state.hub, &id);
    }

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
    screen_id: ScreenId,
    mut rx: tokio::sync::mpsc::Receiver<ScreenEnvelope>,
    mut sink: futures_util::stream::SplitSink<WebSocket, Message>,
) {
    while let Some(envelope) = rx.recv().await {
        let json = match serde_json::to_string(&envelope) {
            Ok(j) => j,
            Err(e) => {
                error!(screen = %screen_id, error = %e, "failed to serialize envelope");
                continue;
            }
        };

        if let Err(e) = sink.send(Message::Text(json.into())).await {
            warn!(screen = %screen_id, error = %e, "failed to send to screen, closing");
            break;
        }
    }

    info!(screen = %screen_id, "write loop ended (channel closed)");
}
