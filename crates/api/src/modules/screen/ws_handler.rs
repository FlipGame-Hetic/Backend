use axum::extract::ws::{Message, WebSocket};
use axum::extract::{Path, Query, State, WebSocketUpgrade};
use axum::response::IntoResponse;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use shared::events::BumperHit;
use shared::screen::{ScreenEnvelope, ScreenId};
use tracing::{debug, error, info, warn};

use crate::errors::ApiError;
use crate::state::AppState;

use super::auth;

/// Query parameter for the JWT token on WS upgrade.
#[derive(Debug, Deserialize)]
pub struct TokenQuery {
    pub token: String,
}

/// `GET /ws/screen/:screen_id?token=xxx`
///
/// Upgrades HTTP to WebSocket after verifying the JWT.
/// The token must contain a `screen_id` claim matching the URL path.
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

/// Full lifecycle of one screen WebSocket connection.
///
/// 1. Register in the `ScreenRegistry` (rejects if already connected).
/// 2. Spawn two tasks:
///    - **read**: parse `ScreenEnvelope` from WS frames, dispatch via `ScreenRouter`.
///    - **write**: drain the `ScreenHandle.rx` channel and send frames to the client.
/// 3. On disconnect, the `ScreenHandle` is dropped which auto-unregisters.
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

    // `handle` is dropped here -> auto-unregister from the registry.
    info!(screen = %screen_id, "screen websocket disconnected");
}

/// Read frames from the screen, deserialize as `ScreenEnvelope`, and dispatch.
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

        // Safety: verify the `from` field matches the authenticated screen.
        if envelope.from != screen_id {
            warn!(
                screen = %screen_id,
                claimed_from = %envelope.from,
                "screen tried to spoof 'from' field, ignoring"
            );
            continue;
        }

        if envelope.event_type == "Bumper" {
            match serde_json::from_value::<BumperHit>(envelope.payload.clone()) {
                Ok(hit) => {
                    info!(
                        screen = %screen_id,
                        bumper_id = hit.bumper_id,
                        "bumper hit received"
                    );
                }
                Err(e) => {
                    warn!(screen = %screen_id, error = %e, "invalid BumperHit payload");
                    continue;
                }
            }
        }

        let result = state.screen_router.dispatch(envelope).await;

        debug!(
            screen = %screen_id,
            delivered = result.delivered,
            missed = ?result.missed,
            intercepted = result.intercepted,
            "dispatch result"
        );
    }
}

/// Drain the per-screen channel and forward envelopes as JSON text frames.
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
