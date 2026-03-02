# Flipper Pinball Backend

Rust backend for the connected pinball system. ESP32 (MQTT) <-> Central API (WebSocket) communication.

## Architecture

```
crates/
├── api/            # HTTP + WebSocket server (Axum)
├── mqtt-bridge/    # Bidirectional relay MQTT <-> WebSocket
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
| Scalar    | 8080 | OpenAPI docs (`/docs`)              |

## Local dev (without Docker)

```bash
rustup default stable       # Rust 1.89+
cargo build                 # Build workspace
cargo test                  # 16 tests (shared + api + bridge)
```

## Tech stack

| Dep                                | Role                         |
|------------------------------------|------------------------------|
| Rust edition **2024** / resolver 3 | Toolchain                    |
| **Axum** 0.8                       | HTTP + WebSocket server      |
| **rumqttc** 0.25                   | Async MQTT client            |
| **tokio-tungstenite**              | WebSocket client (bridge)    |
| **utoipa** + **utoipa-scalar**     | OpenAPI spec + Scalar UI     |
| **tracing**                        | Structured JSON logging      |
| **cargo-chef**                     | Docker layer caching         |

## MQTT Topics

`pinball/<device_id>/<subtopic>`

---

Inbound: `input/button`, `input/plunger`, `input/gyro`, `telemetry`, `events`, `status`

---
Outbound: `ball/hit`, `game/state`, `cmd`
