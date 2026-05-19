# Flipper Pinball Backend

Rust backend for the connected pinball system. ESP32 (MQTT) <-> Central API (WebSocket) communication.

## If you work on my backend, please respect the following rules
- 1 feature = add unit tests
- Follow DRY & SOLID principles
- Write clean code (meaningful variable names, use named constants, etc.)
- Add inline comments `//` and doc comments `///` for important parts

### PR & commit rules
- One PR = one concern
- Commit messages must be descriptive (please follow the convention)


## Architecture

```
crates/
├── api/            # HTTP + WebSocket server (Axum) — REST routes, JWT auth, Lucyd docs
├── game-logic/     # Game state machine and scoring engine
├── mqtt-bridge/    # Bidirectional relay MQTT <-> WebSocket
├── screen-hub/     # Bidirectional relay Frontend apps <-> Backend
└── shared/         # Types, events, DTOs shared across crates
```
## Quickstart

```bash
docker compose up --build -d          # Full stack
docker compose up -d mosquitto        # Broker only (for MQTT Explorer)
```
| Service   | Port | Description                         |
|-----------|------|-------------------------------------|
| API       | 8080 | REST + WS (`/ws/bridge`, `/health`) |
| Mosquitto | 1883 | MQTT broker (anonymous)             |
| Nginx     | 80   | Reverse proxy                       |
| Lucyd   | 8080 | Interactive API docs (`/docs`)     |

## Local dev (without Docker)

```bash
rustup default stable       # Rust 1.89+
cargo build                 # Build workspace
cargo test                  # 16 tests (shared + api + bridge)
```
Documentation of the codebase:
```bash
cargo doc --open
```
## How to run the tests (with Cargo)

Installation
```
cargo install cargo-llvm-cov
```
Run the tests + report in the terminal
```
cargo llvm-cov
```
HTML report + Tab of the coverage
```
cargo llvm-cov --html
```
(Last testing 15/04/2026: ~61.5% codecoverage)


## Tech stack

| Dep                                | Role                                      |
|------------------------------------|-------------------------------------------|
| Rust edition **2024** / resolver 3 | Toolchain                                 |
| **Axum** 0.8                       | HTTP + WebSocket server                   |
| **rumqttc** 0.25                   | Async MQTT client                         |
| **tokio-tungstenite** 0.26         | WebSocket client (bridge)                 |
| **lucyd** ≥ 0.1.9                  | Interactive OpenAPI docs UI (`/docs`)     |
| **utoipa** 5.4                     | OpenAPI spec generation (Axum integration)|
| **sqlx** 0.8                       | Async SQLite driver with migrations       |
| **jsonwebtoken** 9                 | JWT encoding / validation                 |
| **tower-http** 0.6                 | CORS + request tracing middleware         |
| **thiserror** 2                    | Ergonomic error types                     |
| **schemars** 0.8                   | JSON Schema generation                    |
| **tracing** + **tracing-subscriber**| Structured JSON logging                  |
| **cargo-chef**                     | Docker layer caching                      |

## WebSocket — Screen messaging

All frontend screens (`front_screen`, `back_screen`, `dmd_screen`) connect to the backend via WebSocket. Every message exchanged is a `ScreenEnvelope` serialized as JSON.

### Envelope structure

```json
{
  "from": "front_screen",
  "to": { "kind": "screen", "id": "back_screen" },
  "event_type": "game_state_update",
  "payload": { "score": 42000, "combo": 3 }
}
```

| Field        | Type                          | Description                                              |
|--------------|-------------------------------|----------------------------------------------------------|
| `from`       | `ScreenId`                    | Sender — `front_screen`, `back_screen`, or `dmd_screen`  |
| `to`         | `ScreenTarget`                | `{"kind":"screen","id":"<id>"}` or `{"kind":"broadcast"}`|
| `event_type` | `string`                      | Free-form event name (e.g. `game_state_update`)          |
| `payload`    | `any JSON`                    | Arbitrary data, no fixed schema (intentionally flexible) |

### How it flows

```
Screen WS client
      │  JSON text frame (ScreenEnvelope)
      ▼
  WS handler  ──deserialise──►  ScreenRouter
                                     │
                              Interceptor chain
                              (validate / enrich / swallow)
                                     │
                               ScreenRegistry
                              (mpsc per screen, cap 128)
                                     │
                          ┌──────────┴──────────┐
                     target screen         all others
                    (unicast)              (broadcast)
```

Key behaviours:
- A screen can only have **one active connection** — duplicate connections are rejected.
- **Broadcast** delivers to every connected screen except the sender.
- If a screen's channel is full (128 messages), the message is **dropped** and a warning is logged.
- When the WS connection closes, the `ScreenGuard` auto-unregisters the screen from the registry.
- Interceptors run before dispatch and can mutate or swallow any message.

## Docker Publish (GHCR)

Images are automatically built and pushed to `ghcr.io/flipgame-hetic/backend` on every push to `main` or `dev`, and on each GitHub release. The `latest` tag always tracks `main`; branches and releases get their own tags (branch name, semver, short SHA). Build cache is stored in the registry itself (`buildcache` tag) to speed up subsequent runs.

## MQTT Topics

`pinball/<device_id>/<subtopic>`

---

Inbound: `input/button`, `input/plunger`, `input/gyro`, `telemetry`, `events`, `status`

---
Outbound: `ball/hit`, `game/state`, `cmd`
