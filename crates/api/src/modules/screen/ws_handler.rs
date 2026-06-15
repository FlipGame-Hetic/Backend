use axum::extract::ws::{Message, WebSocket};
use axum::extract::{Path, Query, State, WebSocketUpgrade};
use axum::response::IntoResponse;
use futures_util::{SinkExt, StreamExt};
use lucyd::lucy_ws;
use serde::Deserialize;
use shared::screen::{ScreenEnvelope, ScreenEventType, ScreenId};
use tracing::{debug, error, info, warn};

use crate::errors::ApiError;
use crate::modules::game::service::{GameService, GameServiceError};
use crate::state::AppState;

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
    match &envelope.event_type {
        ScreenEventType::StartGame => {
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

            match GameService::new(state).start(player_id, character_id).await {
                Ok(_) => {}
                Err(GameServiceError::AlreadyInProgress) => {
                    warn!("StartGame ignored — game already in progress");
                }
                Err(e) => {
                    error!(error = %e, "game service error starting game from screen");
                }
            }
        }
        ScreenEventType::RailStart => {
            let ball_id = extract_ball_id(&envelope.payload);
            GameService::new(state).start_rail(ball_id, false).await;
        }
        ScreenEventType::RailEnd => {
            let ball_id = extract_ball_id(&envelope.payload);
            GameService::new(state).end_rail(ball_id, false).await;
        }
        ScreenEventType::RampStart => {
            let ball_id = extract_ball_id(&envelope.payload);
            GameService::new(state).start_rail(ball_id, true).await;
        }
        ScreenEventType::RampEnd => {
            let ball_id = extract_ball_id(&envelope.payload);
            GameService::new(state).end_rail(ball_id, true).await;
        }
        _ => {
            if let Err(e) = GameService::new(state).process_screen_event(envelope).await {
                error!(error = %e, "game service error processing screen event");
            }
        }
    }
}

fn extract_ball_id(payload: &serde_json::Value) -> Option<u8> {
    payload
        .get("ball_id")
        .and_then(|v| v.as_u64())
        .and_then(|v| u8::try_from(v).ok())
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
