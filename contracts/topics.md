# Topic & Payload

Prefix: `pinball/<deviceId>/`

| Topic | Description | Sent by | Payload |
|-------|:-----------:|:-------:|---------|
| `input/button` | button state (press/release) | ESP32 | JSON button |
| `input/plunger` | plunger position + release | ESP32 | JSON plunger |
| `input/gyro` | gyroscope / tilt detection | ESP32 | JSON gyro |
| `ball/hit` | active collisions list | Server | JSON hit |
| `game/state` | game state (retain) | Server | JSON state |
| `telemetry` | metrics (rssi, uptime, heap) | ESP32 | JSON telemetry |
| `events` | events (boot, ack, alert, error) | ESP32 | JSON event |
| `cmd` | commands (vibrate, reboot, ota) | Server | JSON cmd |
| `status` | status (retain) | ESP32 | JSON status |

---

## Payloads

### `input/button`

```json
{
  "id": "flipper_left",
  "state": 1,
  "ts": 84200
}
```

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | `flipper_left`, `flipper_right`, `start`, `extra_1`, `extra_2` |
| `state` | int | `1` = pressed, `0` = released |
| `ts` | int | ESP32 `millis()` timestamp |

Each button sends its own independent message. Simultaneous presses = multiple messages a few µs apart.

---

### `input/plunger`

```json
{
  "position": 0.75,
  "released": false,
  "ts": 84200
}
```

| Field | Type | Description |
|-------|------|-------------|
| `position` | float | Pull distance (0.0 = rest, 1.0 = fully pulled) |
| `released` | bool | `true` on the frame the player lets go (launch trigger) |
| `ts` | int | ESP32 `millis()` timestamp |

Sent continuously while pulled (~30Hz). On release, one final message with `released: true` so the server can calculate launch force from last `position`.

---

### `input/gyro` - ~20Hz

```json
{
  "ax": 0.02,
  "ay": -0.15,
  "az": 9.78,
  "tilt": false
}
```

| Field | Type | Description |
|-------|------|-------------|
| `ax` | float | Acceleration X axis (g) |
| `ay` | float | Acceleration Y axis (g) |
| `az` | float | Acceleration Z axis (g) |
| `tilt` | bool | `true` if threshold exceeded (ESP32 side detection) |

ESP32 handles tilt detection locally. Raw values sent so 3JS can apply a nudge effect on physics.

---

### `ball/hit`

A flat array of objects currently being hit by the ball. Sent each time a collision starts or ends.

```json
{
  "hits": [
    { "id": "bumper_1", "type": "bumper", "force": 0.95 },
    { "id": "rail_left", "type": "rail", "force": 0.30 }
  ]
}
```

Empty array = no active collision, ESP32 stops all vibration.

```json
{
  "hits": []
}
```

| Field | Type | Description |
|-------|------|-------------|
| `hits` | array | List of active collisions |
| `hits[].id` | string | Unique object identifier (matches 3JS mesh name) |
| `hits[].type` | string | `bumper`, `rail`, `slingshot`, `drain`, `target`, `spinner` |
| `hits[].force` | float | Impact intensity (0.0 → 1.0) |

**Vibration profiles (ESP32 side):**

| Type | Vibration profile |
|------|-------------------|
| `bumper` | Sharp pulse 80ms, max intensity, hard cutoff |
| `rail` | Soft continuous vibration, proportional to force |
| `slingshot` | Fast ramp 20ms then decay 150ms |
| `drain` | Long rumble 500ms, low frequency |
| `target` | Double tap: 40ms on / 30ms off / 40ms on |
| `spinner` | Rapid repeated bursts, decreasing intensity |

ESP32 maps each `id` to the nearest vibrator(s) via a static lookup table.

---

### `game/state` - retain: true

```json
{
  "state": "playing",
  "ball_number": 2,
  "score": 1250000,
  "player": 1,
  "total_players": 2
}
```

| Field | Type | Description |
|-------|------|-------------|
| `state` | string | `idle`, `attract`, `start`, `playing`, `ball_lost`, `bonus`, `tilt`, `game_over`, `high_score` |
| `ball_number` | int | Current ball number |
| `score` | int | Current score |
| `player` | int | Active player |
| `total_players` | int | Total player count |

---

### `telemetry` - every 10s

```json
{
  "wifi_rssi": -42,
  "uptime_s": 84200,
  "loop_freq_hz": 1000,
  "free_heap": 142000,
  "mqtt_reconnects": 0
}
```

| Field | Type | Description |
|-------|------|-------------|
| `wifi_rssi` | int | WiFi signal strength in dBm |
| `uptime_s` | int | Uptime in seconds |
| `loop_freq_hz` | int | Main loop frequency |
| `free_heap` | int | Free heap memory in bytes |
| `mqtt_reconnects` | int | MQTT reconnection count since boot |

---

### `events`

```json
{
  "event": "boot",
  "fw_version": "1.2.0",
  "reason": "power_on",
  "ts": 1719312000000
}
```

| Field | Type | Description |
|-------|------|-------------|
| `event` | string | `boot`, `ack`, `alert`, `error`, `ota_start`, `ota_done` |
| `fw_version` | string | Current firmware version |
| `reason` | string | Event-specific context |
| `ts` | int | ESP32 timestamp |

---

### `cmd`

```json
{
  "cmd": "vibrate",
  "params": {
    "vibrator": 3,
    "intensity": 200,
    "duration_ms": 100
  }
}
```

| Field | Type | Description |
|-------|------|-------------|
| `cmd` | string | `vibrate`, `reboot`, `ota`, `set_config` |
| `params` | object | Command-specific parameters |

**Available commands:**

| Command | Params | Description |
|---------|--------|-------------|
| `vibrate` | `vibrator`, `intensity`, `duration_ms` | Manual vibrator test |
| `reboot` | — | Reboot ESP32 |
| `ota` | `url` | Trigger OTA firmware update |
| `set_config` | key/value pairs | Update runtime config |

---

### `status` - retain: true

```json
{
  "online": true,
  "fw_version": "1.2.0",
  "ip": "192.168.1.50",
  "free_heap": 142000,
  "vibrators_ok": [true, true, true, true, true, true, true, true, true],
  "gyro_ok": true
}
```

| Field | Type | Description |
|-------|------|-------------|
| `online` | bool | Device online status |
| `fw_version` | string | Firmware version |
| `ip` | string | Current IP address |
| `free_heap` | int | Free heap in bytes |
| `vibrators_ok` | bool[] | Health check for each of the 9 vibrators |
| `gyro_ok` | bool | Gyroscope connection status |

---

## Topic tree

```
pinball/<deviceId>/
├── input/
│   ├── button            ESP32 → Server     Player inputs (5 buttons)
│   ├── plunger           ESP32 → Server     Plunger position + release
│   └── gyro              ESP32 → Server     Gyroscope / tilt 20Hz
├── ball/
│   └── hit               Server → ESP32     Active collisions list
├── game/
│   └── state             Server → ESP32     Game state (retain)
├── telemetry             ESP32 → Server     Device metrics
├── events                ESP32 → Server     Lifecycle events
├── cmd                   Server → ESP32     Remote commands
└── status                ESP32 → Server     Device status (retain)
```

---

## Notes

- **No ball position tracking**: the server only sends collision events. ESP32 maps each object `id` to the nearest vibrator(s) via a static lookup table.
- **Plunger**: analog read (potentiometer or hall sensor), sent continuously while pulled. The `released` flag lets the server know when to launch.
- **Gyro tilt**: ESP32 detects tilt locally and flags it. Raw accelerometer data also sent for 3JS nudge effect.
- **Simultaneous buttons**: each button fires its own message independently via GPIO interrupts.
- **Retain**: only `game/state` and `status` use retain for reconnection recovery.
- **LWT**: ESP32 registers a Last Will on `status` with `{"online": false}` for disconnect detection.
