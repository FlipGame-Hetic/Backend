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
├── api/            # HTTP + WebSocket server (Axum)
├── mqtt-bridge/    # Bidirectional relay MQTT <-> WebSocket
├── screen-hub/    # Bidirectional relay Frontend apps <-> Frontend app & Frontend apps <-> Backend
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
| Lucy    | 8080 | Custom docs (`/docs`)              |

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
