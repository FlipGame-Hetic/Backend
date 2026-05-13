use shared::events::WsMessage;
use tokio::sync::broadcast;
use tracing::debug;

/// Channel capacity for the broadcast relay.
const BROADCAST_CAPACITY: usize = 256;

/// Central hub that relays `WsMessage` frames between all connected bridges.
///
/// Each bridge WebSocket connection subscribes to the broadcast channel.
/// - **Inbound** messages (bridge -> API) are received per-connection in the handler
///   and can be forwarded to game logic or logged.
/// - **Outbound** messages (API -> bridge) are sent via `broadcast` so every
///   connected bridge receives them and can route to the correct device over MQTT.
#[derive(Debug, Clone)]
pub struct BridgeHub {
    tx: broadcast::Sender<WsMessage>,
}

impl Default for BridgeHub {
    fn default() -> Self {
        Self::new()
    }
}

impl BridgeHub {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(BROADCAST_CAPACITY);
        debug!(capacity = BROADCAST_CAPACITY, "bridge hub created");
        Self { tx }
    }

    /// Subscribe to outbound messages. Each bridge connection calls this once.
    pub fn subscribe(&self) -> broadcast::Receiver<WsMessage> {
        self.tx.subscribe()
    }

    /// Send an outbound message to all connected bridges (fire-and-forget).
    pub fn send(&self, msg: WsMessage) {
        let _ = self.tx.send(msg);
    }
}
