# WebSocket API

OpenAPI / Scalar does not support WebSocket endpoints. This file documents both WS endpoints exposed by the API.

---

## Overview

| Endpoint | Used by | Purpose |
|---|---|---|
| `GET /ws/bridge` | `mqtt-bridge` service only | Relay MQTT events between ESP32 devices and the API |
| `GET /ws/screen/{screen_id}` | Frontend apps | Screen-to-screen communication through the backend |

> **Frontend apps must only connect to `/ws/screen/{screen_id}`.  
> `/ws/bridge` is an internal service endpoint, do not connect to it from a browser.**

---

## `/ws/screen/{screen_id}` - Frontend screens

This is the endpoint each frontend app connects to. The backend acts as a message broker: a screen sends an envelope, the backend routes it to the target screen(s).

```
front-screen  ──►  /ws/screen/front_screen  ──►  backend  ──►  /ws/screen/dmd_screen  ──►  dmd-screen
```

### Step 1 - Get a JWT token

Every connection requires a JWT passed as a query parameter. The token identifies which screen is connecting and must match the URL path.

Generate tokens for all screens (run from the `Backend/` directory):

```bash
cargo test -p api print_all_screen_tokens -- --nocapture
```

Output:

```
# front_screen
VITE_SCREEN_TOKEN=eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9...

# back_screen
VITE_SCREEN_TOKEN=eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9...

# dmd_screen
VITE_SCREEN_TOKEN=eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9...
```

Paste each token into the corresponding app's `.env` file:

```
# apps/front-screen/.env
VITE_SCREEN_TOKEN=eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9...

# apps/dmd-screen/.env
VITE_SCREEN_TOKEN=eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9...

# apps/back-screen/.env
VITE_SCREEN_TOKEN=eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9...
```

> Tokens are not tied to a session and do not expire, they are device-identity tokens.  
> Each token is only valid for its own screen: a `front_screen` token will be rejected on `/ws/screen/dmd_screen`.

> In production, set the `SCREEN_JWT_SECRET` env var (min. 32 characters) on the backend before generating tokens.

### Step 2 - Connect

```
ws://localhost:8080/ws/screen/{screen_id}?token={VITE_SCREEN_TOKEN}
```

Available `screen_id` values:

| `screen_id` | App | Description |
|---|---|---|
| `front_screen` | `apps/front-screen` | Main screen, 3D physics simulation |
| `back_screen` | `apps/back-screen` | Back screen, terminal log |
| `dmd_screen` | `apps/dmd-screen` | DMD, pixel art score display |

Both kebab-case (`front-screen`) and snake_case (`front_screen`) are accepted in the URL.

**If the token is invalid or mismatched**, the server rejects the WebSocket upgrade with HTTP `400 Bad Request`, the connection is never established.

**If the screen is already connected** (duplicate session), the new connection is closed immediately server-side.

### Step 3 - Send a message

All messages use the `ScreenEnvelope` format:

```json
{
  "from": "front_screen",
  "to": { "kind": "screen", "id": "dmd_screen" },
  "event_type": "game_state_update",
  "payload": { "score": 1250000, "ball_number": 2, "player": 1, "total_players": 2, "state": "playing" }
}
```

| Field | Type | Description |
|---|---|---|
| `from` | `ScreenId` | **Must be set by the client** to its own screen id. The server rejects messages where `from` does not match the authenticated screen. |
| `to` | `ScreenTarget` | Routing target, see below |
| `event_type` | `string` | Event type identifier, see conventions below |
| `payload` | `object` | Event data, free-form JSON |

#### Routing target (`to`)

Send to a specific screen:
```json
{ "kind": "screen", "id": "dmd_screen" }
```

Broadcast to all connected screens except the sender:
```json
{ "kind": "broadcast" }
```

#### `event_type` conventions

Use these strings to keep event types consistent across apps:

| `event_type` | Sender | Receivers | Payload fields |
|---|---|---|---|
| `game_state_update` | `front_screen` | `dmd_screen`, `back_screen` | `score`, `ball_number`, `player`, `total_players`, `state` |
| `ball_hit` | `front_screen` | broadcast | `hits: [{id, type, force}]` |
| `ping` | any | any | `{}` |

> `state` values: `idle`, `attract`, `start`, `playing`, `ball_lost`, `bonus`, `tilt`, `game_over`, `high_score`

### Step 4 - Receive messages

The backend delivers incoming envelopes to the target screen over the same WebSocket connection. The received message has the same `ScreenEnvelope` format, parse it to get the `event_type` and `payload`.

```
another screen sends → backend routes → your WS connection receives the envelope as a JSON text frame
```

A screen only receives messages that are addressed to it (either `{ "kind": "screen", "id": "<your_id>" }` or `{ "kind": "broadcast" }`). It never receives its own messages.


### Connection lifecycle

```
1. HTTP GET /ws/screen/front_screen?token=...
2. Server validates JWT, 400 if invalid
3. Server registers the screen, closes connection if already connected
4. WebSocket is open, send and receive ScreenEnvelope frames
5. On disconnect, server automatically unregisters the screen (no action needed)
```

---

## `/ws/bridge` - Internal only

This endpoint is used exclusively by the `mqtt-bridge` service to relay events between ESP32 devices and the API. **Do not connect to this from a frontend app.**

All messages use a `WsMessage` envelope tagged on the `"dir"` field.

### Bridge to API (`dir: "inbound"`)

MQTT event from an ESP32, forwarded by the bridge.

```json
{
  "dir": "inbound",
  "device_id": "pinball-01",
  "_type": "Button",
  "id": "flipper_left",
  "state": 1,
  "ts": 84200
}
```

| `_type` | Description |
|---|---|
| `Button` | Button press / release |
| `Plunger` | Plunger position and release |
| `Gyro` | Accelerometer / tilt detection |
| `Telemetry` | Device metrics (rssi, heap, uptime) |
| `Event` | Lifecycle events (boot, OTA…) |
| `Status` | Device status (retained) |

### API to Bridge (`dir: "outbound"`)

Command targeting an ESP32, sent by the API via the hub broadcast.

```json
{
  "dir": "outbound",
  "device_id": "pinball-01",
  "_type": "BallHit",
  "hits": [{ "id": "bumper_1", "type": "bumper", "force": 0.95 }]
}
```

| `_type` | Description |
|---|---|
| `BallHit` | Active collision list |
| `GameState` | Current game state (retained) |
| `Command` | Direct ESP32 command (vibrate, reboot, OTA…) |

---

## Manual testing

### websocat

```bash
# Connect as front_screen (replace TOKEN)
websocat "ws://localhost:8080/ws/screen/front_screen?token=TOKEN"

# Send a message once connected (type in the terminal)
{"from":"front_screen","to":{"kind":"screen","id":"dmd_screen"},"event_type":"ping","payload":{}}
```

### REST debug endpoints (no WS client needed)

Inject a `ScreenEnvelope` without opening a WS connection:

```bash
curl -X POST http://localhost:8080/api/v1/screens/send \
  -H "Content-Type: application/json" \
  -d '{
    "from": "front_screen",
    "to": { "kind": "screen", "id": "dmd_screen" },
    "event_type": "game_state_update",
    "payload": { "score": 42000, "ball_number": 1, "player": 1, "total_players": 1, "state": "playing" }
  }'
```

List currently connected screens:

```bash
curl http://localhost:8080/api/v1/screens/connected
```
