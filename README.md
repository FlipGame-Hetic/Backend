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
| [Lucyd](https://github.com/Jeck0v/Lucyd)   | 8080 | Interactive API docs (`/docs`)     |

## Local dev (without Docker)

```bash
rustup default stable       # Rust 1.89+
cargo build                 # Build workspace
cargo test                  # 158 tests (api + game-logic + screen-hub + mqtt-bridge + shared)
```
Documentation of the codebase:
```bash
cargo doc --open
```

## Environment variables

| Variable           | Required | Default                      | Description                                        |
|--------------------|----------|------------------------------|----------------------------------------------------|
| `SCREEN_JWT_SECRET`| **yes**  | —                            | JWT signing secret (min 32 chars)                  |
| `DATABASE_URL`     | no       | `sqlite:///data/flipper.db`  | SQLite database path                               |
| `API_PORT`         | no       | `8080`                       | HTTP listen port                                   |
| `ALLOWED_ORIGINS`  | no       | `http://localhost:3000`      | Comma-separated CORS allowed origins               |

## Admin token

Admin routes (`/api/v1/admin/…`) require a `Authorization: Bearer <token>` header where the token is a JWT signed with `SCREEN_JWT_SECRET` and carrying `role: "admin"`.

Generate a token with the built-in CLI command:

```bash
# Local
SCREEN_JWT_SECRET='your-secret' cargo run -p api -- generate-admin-token

# Docker (secret is already injected via the container env)
docker compose run --rm api generate-admin-token

# Or inside a running container
docker exec <container_name> /app/api generate-admin-token
```

Output: `ADMIN_TOKEN=eyJ…` — copy the value and use it as the Bearer token.

> The token has no expiry. Keep it secret; rotate by regenerating with a new `SCREEN_JWT_SECRET`.

## REST API

| Method | Path                | Description                                                  |
|--------|---------------------|--------------------------------------------------------------|
| `GET`  | `/health`           | Liveness probe                                               |
| `POST` | `/api/v1/game/start`| Start a new game session (`{ character_id: u8 }`)           |
| `GET`  | `/api/v1/game/state`| Current game state (404 if no game running)                  |
| `POST` | `/api/v1/game/end`  | Force-end current game and persist the final score           |
| `GET`  | `/api/v1/scores`    | Top-10 all-time leaderboard (sorted by score desc)           |
| `POST` | `/api/v1/scores`    | Debug: attempt to insert a score into the leaderboard        |
| `GET`  | `/docs`             | Interactive OpenAPI docs (Lucyd UI)                          |

### Leaderboard logic

Scores are stored in SQLite (`scores` table). The board is capped at **10 entries**:
- If fewer than 10 entries exist, every score is inserted.
- If the board is full, the score must be **strictly greater** than the current minimum to be accepted; the minimum entry is then evicted atomically.
- The leaderboard is automatically broadcast to `back_screen` via a `LeaderboardUpdate` WebSocket envelope at the end of every game.

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
(Last tested 18/06/2026: ~68.55% code coverage)


## Tech stack

| Dep                                | Role                                      |
|------------------------------------|-------------------------------------------|
| Rust edition **2024** / resolver 3 | Toolchain                                 |
| **Axum** 0.8                       | HTTP + WebSocket server                   |
| **rumqttc** 0.25                   | Async MQTT client                         |
| **tokio-tungstenite** 0.26         | WebSocket client (bridge)                 |
| **lucyd** ≥ 0.1.9                  | Interactive OpenAPI docs UI (`/docs`)     |
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

### Known event types

| `event_type`        | Direction          | Description                                              |
|---------------------|--------------------|----------------------------------------------------------|
| `game_state_update` | server → screen    | Current game state snapshot                              |
| `game_over`         | server → screen    | Game ended                                               |
| `boss_defeated`     | server → screen    | A boss was defeated                                      |
| `leaderboard_update`| server → back_screen | Top-10 leaderboard payload, broadcast after each game  |

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

Inbound: `input/button`, `input/under_plunger`, `input/plunger`, `input/gyro`, `telemetry`, `events`, `status`

---
Outbound: `ball/hit`, `game/state`, `cmd`
