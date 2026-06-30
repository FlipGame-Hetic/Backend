//! Integration tests for the `/ws/bridge` WebSocket endpoint.
//!
//! Each test starts a real Axum server on an ephemeral port, connects one or
//! more WebSocket clients, and verifies end-to-end message routing across the
//! full stack (WS → game engine → hub → WS out).
//!
//! No MQTT broker is needed: the bridge WS endpoint is the boundary under test.
//! The MQTT parsing layer is covered by `mqtt-bridge` unit tests.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use reqwest::Client;
use serde_json::{Value, json};
use shared::events::{GameState, GyroInput, InboundMessage, OutboundMessage, Telemetry, WsMessage};
use shared::model::{GamePhase, HitType};
use shared::screen::{ScreenEnvelope, ScreenEventType, ScreenId, ScreenTarget};
use sqlx::SqlitePool;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;

// Constants

/// Must match the value set for `SCREEN_JWT_SECRET` in `build_app` below.
const TEST_SECRET: &[u8] = b"flipper-dev-secret-change-in-prod-test";

/// Fake device-id used by bridge helper functions.
const DEVICE_ID: &str = "test-esp01";

// Infrastructure

async fn test_pool() -> SqlitePool {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    pool
}

/// Build a test Axum app using `ApiConfig::from_map` to avoid any env-var
/// races between parallel tests.
fn build_app(pool: SqlitePool) -> axum::Router {
    let mut vars = HashMap::new();
    vars.insert(
        "SCREEN_JWT_SECRET".to_owned(),
        String::from_utf8(TEST_SECRET.to_vec()).unwrap(),
    );
    vars.insert("ALLOWED_ORIGINS".to_owned(), "*".to_owned());
    let config = api::config::ApiConfig::from_map(&vars).unwrap();
    api::app::build(&config, pool)
}

/// Bind an ephemeral port, spawn the server as a background task, and return
/// the bound address together with a reusable HTTP client.
async fn start_server() -> (SocketAddr, Client) {
    let pool = test_pool().await;
    let app = build_app(pool);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (addr, Client::new())
}

type WsStream =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// Open a WebSocket to `/ws/bridge` (no auth required).
async fn connect_bridge(addr: SocketAddr) -> WsStream {
    let url = format!("ws://127.0.0.1:{}/ws/bridge", addr.port());
    let (ws, _) = tokio_tungstenite::connect_async(url).await.unwrap();
    ws
}

/// Mint a screen JWT signed with `TEST_SECRET`.
///
/// The `screen_id` string must match one of the `ScreenId` serde variants
/// (e.g. `"front_screen"`, `"back_screen"`, `"dmd_screen"`).
fn screen_jwt(screen_id: &str) -> String {
    let claims = json!({ "screen_id": screen_id, "sub": screen_id });
    jsonwebtoken::encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(TEST_SECRET),
    )
    .unwrap()
}

/// Open a WebSocket to `/ws/screen/{screen_id}?token=<jwt>`.
async fn connect_screen(addr: SocketAddr, screen_id: ScreenId) -> WsStream {
    let token = screen_jwt(screen_id.as_str());
    let url = format!(
        "ws://127.0.0.1:{}/ws/screen/{}?token={}",
        addr.port(),
        screen_id.as_str(),
        token
    );
    let (ws, _) = tokio_tungstenite::connect_async(url).await.unwrap();
    ws
}

/// Encode a payload as a `WsMessage::Inbound` text frame.
fn bridge_inbound(payload: InboundMessage) -> Message {
    let msg = WsMessage::Inbound {
        device_id: DEVICE_ID.to_owned(),
        payload,
    };
    Message::Text(serde_json::to_string(&msg).unwrap().into())
}

/// Encode a `ScreenEnvelope` as a text frame, ready to send over screen WS.
fn screen_frame(from: ScreenId, event_type: ScreenEventType, payload: Value) -> Message {
    let env = ScreenEnvelope {
        from,
        to: ScreenTarget::Broadcast,
        event_type,
        payload,
    };
    Message::Text(serde_json::to_string(&env).unwrap().into())
}

/// Send a no-op Telemetry inbound from the bridge so the server registers the
/// `active_device_id`.  Without this, the server skips game-state outbound
/// syncs (it doesn't know which device to target).
async fn register_device(bridge: &mut WsStream) {
    bridge
        .send(bridge_inbound(InboundMessage::Telemetry(Telemetry {
            wifi_rssi: -55,
            uptime_s: 1,
            loop_freq_hz: 1000,
            free_heap: 150_000,
            mqtt_reconnects: 0,
        })))
        .await
        .unwrap();
    // Let the server process the inbound before the next step.
    tokio::time::sleep(Duration::from_millis(30)).await;
}

/// Read the next `WsMessage` from the bridge WS, timing out after 2 s.
async fn recv_outbound(ws: &mut WsStream) -> WsMessage {
    let frame = timeout(Duration::from_secs(2), ws.next())
        .await
        .expect("timed out waiting for bridge outbound")
        .expect("bridge WS stream ended unexpectedly")
        .expect("bridge WS read error");

    let text = match frame {
        Message::Text(t) => t.as_str().to_owned(),
        other => panic!("expected Text frame, got {other:?}"),
    };

    serde_json::from_str::<WsMessage>(&text).expect("bridge sent invalid WsMessage JSON")
}

// HTTP convenience wrappers

async fn http_post(client: &Client, url: String, body: Value) -> (u16, Value) {
    let resp = client.post(url).json(&body).send().await.unwrap();
    let status = resp.status().as_u16();
    let json = resp.json::<Value>().await.unwrap_or(Value::Null);
    (status, json)
}

async fn http_get(client: &Client, url: String) -> (u16, Value) {
    let resp = client.get(url).send().await.unwrap();
    let status = resp.status().as_u16();
    let json = resp.json::<Value>().await.unwrap_or(Value::Null);
    (status, json)
}

async fn start_game(client: &Client, addr: SocketAddr) -> (u16, Value) {
    http_post(
        client,
        format!("http://127.0.0.1:{}/api/v1/game/start", addr.port()),
        json!({ "character": "enforcer" }),
    )
    .await
}

async fn game_state(client: &Client, addr: SocketAddr) -> (u16, Value) {
    http_get(
        client,
        format!("http://127.0.0.1:{}/api/v1/game/state", addr.port()),
    )
    .await
}

// Tests

/// Starting a game must push `GameState { state: playing }` to any bridge
/// that is already connected.
///
/// Flow: connect bridge → register device → POST /game/start → read outbound.
#[tokio::test]
async fn game_start_pushes_game_state_to_bridge() {
    let (addr, client) = start_server().await;

    // Subscribe to the hub BEFORE the game starts so we don't miss the broadcast.
    let mut bridge = connect_bridge(addr).await;
    register_device(&mut bridge).await;

    let (status, body) = start_game(&client, addr).await;
    assert_eq!(status, 200);
    assert_eq!(body["phase"], "in_game");

    let msg = recv_outbound(&mut bridge).await;
    match msg {
        WsMessage::Outbound {
            device_id,
            payload: OutboundMessage::GameState(gs),
        } => {
            assert_eq!(device_id, DEVICE_ID);
            assert_eq!(gs.state, GamePhase::Playing);
            assert_eq!(gs.score, 0);
        }
        other => panic!("expected Outbound(GameState), got {other:?}"),
    }
}

/// A gyro-tilt inbound must trigger a score penalty (not a life loss) and push
/// a `GameState` outbound, confirming the full inbound → engine → outbound circuit.
#[tokio::test]
async fn inbound_gyro_tilt_pushes_game_state_to_bridge() {
    let (addr, client) = start_server().await;

    let (status, _) = start_game(&client, addr).await;
    assert_eq!(status, 200);

    // Bridge connects AFTER game start → missed the initial GameState.
    // The device_id is set by the first inbound, which is the tilt itself.
    let mut bridge = connect_bridge(addr).await;

    bridge
        .send(bridge_inbound(InboundMessage::Gyro(GyroInput {
            ax: 2.5,
            ay: 2.5,
            az: 9.8,
            tilt: true,
        })))
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(50)).await;

    // Tilt applies a score penalty — lives are unaffected.
    let (status, body) = game_state(&client, addr).await;
    assert_eq!(status, 200);
    assert_eq!(
        body["phase"], "in_game",
        "game must still be running after tilt"
    );
    assert_eq!(body["lives"], 3, "tilt must not decrement lives");

    // The tilt pushed a GameState outbound — verify it arrives on the bridge.
    let msg = recv_outbound(&mut bridge).await;
    assert!(
        matches!(
            msg,
            WsMessage::Outbound {
                payload: OutboundMessage::GameState(_),
                ..
            }
        ),
        "expected a GameState outbound after tilt"
    );
}

/// A malformed JSON frame must be silently skipped: the connection must stay
/// alive and the server must remain healthy.
#[tokio::test]
async fn inbound_malformed_json_is_skipped() {
    let (addr, client) = start_server().await;
    let mut bridge = connect_bridge(addr).await;

    // Send two bad frames in a row, then a valid telemetry.
    bridge
        .send(Message::Text("{ totally: not json !!! }".into()))
        .await
        .unwrap();
    bridge.send(Message::Text("".into())).await.unwrap();

    tokio::time::sleep(Duration::from_millis(50)).await;

    // Server must still respond normally.
    let (status, _) = game_state(&client, addr).await;
    assert_eq!(status, 404, "no game running, but server should be healthy");

    // Follow up with a valid message to confirm the bridge loop is still running.
    let (status, _) = start_game(&client, addr).await;
    assert_eq!(status, 200, "server should still accept requests");
}

/// A `WsMessage::Outbound` sent from the bridge (wrong direction) must be
/// silently ignored — the server must not start a game or change any state.
#[tokio::test]
async fn inbound_wrong_direction_is_ignored() {
    let (addr, client) = start_server().await;
    let mut bridge = connect_bridge(addr).await;

    let wrong_dir = WsMessage::Outbound {
        device_id: DEVICE_ID.to_owned(),
        payload: OutboundMessage::GameState(GameState {
            state: GamePhase::Playing,
            ball_number: 1,
            score: 99_999,
            player: 1,
            total_players: 1,
        }),
    };
    bridge
        .send(Message::Text(
            serde_json::to_string(&wrong_dir).unwrap().into(),
        ))
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(50)).await;

    // No game should be running — the Outbound was correctly ignored.
    let (status, _) = game_state(&client, addr).await;
    assert_eq!(status, 404, "wrong-direction frame must not start a game");
}

/// A `BallHit` event from a screen WebSocket must be forwarded to the bridge
/// as `WsMessage::Outbound { BallHit { hits: [...] } }` with position and force
/// intact.
#[tokio::test]
async fn ball_hit_from_screen_is_forwarded_to_bridge() {
    let (addr, _client) = start_server().await;

    // Bridge subscribes to hub first, then registers the device_id.
    let mut bridge = connect_bridge(addr).await;
    register_device(&mut bridge).await;

    // Connect the screen and give the read loop a moment to start.
    let mut screen = connect_screen(addr, ScreenId::FrontScreen).await;
    tokio::time::sleep(Duration::from_millis(30)).await;

    // Screen sends a BallHit event.
    screen
        .send(screen_frame(
            ScreenId::FrontScreen,
            ScreenEventType::BallHit,
            json!({
                "hits": [
                    {
                        "id": "bumper_1",
                        "type": "bumper",
                        "force": 0.95,
                        "position": { "x": 1.5, "z": 2.3 }
                    },
                    {
                        "id": "slingshot_left",
                        "type": "slingshot",
                        "force": 0.30,
                        "position": { "x": -1.0, "z": 3.0 }
                    }
                ]
            }),
        ))
        .await
        .unwrap();

    // Bridge must receive the forwarded BallHit outbound.
    let msg = recv_outbound(&mut bridge).await;
    match msg {
        WsMessage::Outbound {
            device_id,
            payload: OutboundMessage::BallHit(ball_hit),
        } => {
            assert_eq!(device_id, DEVICE_ID);
            assert_eq!(ball_hit.hits.len(), 2);

            let bumper = &ball_hit.hits[0];
            assert_eq!(bumper.id, "bumper_1");
            assert_eq!(bumper.hit_type, HitType::Bumper);
            assert!((bumper.force - 0.95).abs() < 1e-3, "force mismatch");
            let pos = bumper.position.as_ref().expect("position must be present");
            assert!((pos.x - 1.5).abs() < 1e-3);
            assert!((pos.z - 2.3).abs() < 1e-3);

            let sling = &ball_hit.hits[1];
            assert_eq!(sling.id, "slingshot_left");
            assert_eq!(sling.hit_type, HitType::Slingshot);
        }
        other => panic!("expected Outbound(BallHit), got {other:?}"),
    }
}

/// Each game event that modifies state must produce a `GameState` outbound.
/// This chains: start game → tilt → verify both outbounds arrive in order.
#[tokio::test]
async fn sequential_game_events_all_reach_bridge() {
    let (addr, client) = start_server().await;

    let mut bridge = connect_bridge(addr).await;
    register_device(&mut bridge).await;

    // --- Event 1: game start ---
    let (status, _) = start_game(&client, addr).await;
    assert_eq!(status, 200);

    let first = recv_outbound(&mut bridge).await;
    match &first {
        WsMessage::Outbound {
            payload: OutboundMessage::GameState(gs),
            ..
        } => {
            assert_eq!(gs.state, GamePhase::Playing, "game should be playing");
            assert_eq!(gs.score, 0, "score starts at 0");
        }
        other => panic!("expected GameState after game start, got {other:?}"),
    }

    // --- Event 2: gyro tilt (costs a life) ---
    bridge
        .send(bridge_inbound(InboundMessage::Gyro(GyroInput {
            ax: 2.5,
            ay: 2.5,
            az: 9.8,
            tilt: true,
        })))
        .await
        .unwrap();

    let second = recv_outbound(&mut bridge).await;
    match second {
        WsMessage::Outbound {
            payload: OutboundMessage::GameState(gs),
            ..
        } => {
            assert_eq!(
                gs.state,
                GamePhase::Playing,
                "game still playing after one tilt"
            );
        }
        other => panic!("expected GameState after tilt, got {other:?}"),
    }
}
