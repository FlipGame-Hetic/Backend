use std::collections::HashMap;
use std::sync::Arc;

use shared::screen::{ScreenEnvelope, ScreenId};
use tokio::sync::{RwLock, mpsc};
use tracing::{debug, info, warn};

use crate::error::{Result, ScreenHubError};

/// Capacity for per-screen outbound channels.
const SCREEN_CHANNEL_CAPACITY: usize = 128;

/// Guard that auto-unregisters the screen when dropped.
///
/// Hold this for the lifetime of the WebSocket connection.
/// When the connection ends and the guard is dropped, the screen
/// is automatically removed from the registry.
pub struct ScreenGuard {
    screen_id: ScreenId,
    registry: Arc<ScreenRegistryInner>,
}

impl Drop for ScreenGuard {
    fn drop(&mut self) {
        let inner = Arc::clone(&self.registry);
        let id = self.screen_id;

        // Fire-and-forget cleanup. We spawn because Drop is sync.
        tokio::spawn(async move {
            inner.remove(id).await;
        });
    }
}

/// Handle returned when a screen registers.
///
/// Use `into_parts()` to split into the receiver (for the write loop)
/// and the guard (hold it for the connection lifetime).
pub struct ScreenHandle {
    rx: mpsc::Receiver<ScreenEnvelope>,
    guard: ScreenGuard,
}

impl ScreenHandle {
    /// Split into the message receiver and the cleanup guard.
    ///
    /// The `ScreenGuard` must be held alive for the duration of the connection.
    /// Dropping it triggers automatic unregistration from the registry.
    pub fn into_parts(self) -> (mpsc::Receiver<ScreenEnvelope>, ScreenGuard) {
        (self.rx, self.guard)
    }
}

/// Internal state behind the `Arc`.
#[derive(Default)]
struct ScreenRegistryInner {
    senders: RwLock<HashMap<ScreenId, mpsc::Sender<ScreenEnvelope>>>,
}

impl ScreenRegistryInner {
    async fn remove(&self, id: ScreenId) {
        let removed = self.senders.write().await.remove(&id);
        if removed.is_some() {
            info!(screen = %id, "screen unregistered");
        }
    }
}

/// Thread-safe registry of connected screens.
///
/// Each screen gets an `mpsc` channel when it registers.
/// The registry holds the `Sender` side; the screen WS handler holds the `Receiver`
/// via the returned `ScreenHandle`.
///
/// Routing logic lives in [`crate::router::ScreenRouter`], not here.
/// This struct is purely about connection lifecycle.
#[derive(Clone, Default)]
pub struct ScreenRegistry {
    inner: Arc<ScreenRegistryInner>,
}

impl ScreenRegistry {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(ScreenRegistryInner {
                senders: RwLock::new(HashMap::new()),
            }),
        }
    }

    /// Register a screen and return a handle that receives messages for it.
    ///
    /// Fails if the screen is already connected (no duplicate sessions).
    pub async fn register(&self, id: ScreenId) -> Result<ScreenHandle> {
        let mut senders = self.inner.senders.write().await;

        if senders.contains_key(&id) {
            return Err(ScreenHubError::AlreadyConnected(id));
        }

        let (tx, rx) = mpsc::channel(SCREEN_CHANNEL_CAPACITY);
        senders.insert(id, tx);

        info!(screen = %id, "screen registered");

        let guard = ScreenGuard {
            screen_id: id,
            registry: Arc::clone(&self.inner),
        };

        Ok(ScreenHandle { rx, guard })
    }

    /// Send an envelope to a specific screen.
    ///
    /// Returns `Ok(true)` if sent, `Ok(false)` if the screen is not connected,
    /// or `Err` if the channel is closed (stale entry — cleaned up automatically).
    pub async fn send_to(&self, id: ScreenId, envelope: &ScreenEnvelope) -> Result<bool> {
        let senders = self.inner.senders.read().await;

        let Some(tx) = senders.get(&id) else {
            debug!(screen = %id, "screen not connected, skipping");
            return Ok(false);
        };

        match tx.try_send(envelope.clone()) {
            Ok(()) => Ok(true),
            Err(mpsc::error::TrySendError::Full(_)) => {
                warn!(screen = %id, "screen channel full, dropping message");
                Ok(false)
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                drop(senders);
                // Stale sender — clean up.
                self.inner.remove(id).await;
                Err(ScreenHubError::SendFailed(id))
            }
        }
    }

    /// Send an envelope to all connected screens except `exclude`.
    pub async fn broadcast(&self, envelope: &ScreenEnvelope, exclude: ScreenId) {
        let senders = self.inner.senders.read().await;

        for (&id, tx) in senders.iter() {
            if id == exclude {
                continue;
            }

            match tx.try_send(envelope.clone()) {
                Ok(()) => {
                    debug!(screen = %id, "broadcast delivered");
                }
                Err(mpsc::error::TrySendError::Full(_)) => {
                    warn!(screen = %id, "screen channel full during broadcast, dropped");
                }
                Err(mpsc::error::TrySendError::Closed(_)) => {
                    warn!(screen = %id, "screen channel closed during broadcast");
                    // Cleanup happens when the ScreenHandle is dropped.
                }
            }
        }
    }

    /// Returns the list of currently connected screen ids.
    pub async fn connected_screens(&self) -> Vec<ScreenId> {
        self.inner.senders.read().await.keys().copied().collect()
    }

    /// Check if a specific screen is currently connected.
    pub async fn is_connected(&self, id: ScreenId) -> bool {
        self.inner.senders.read().await.contains_key(&id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::screen::ScreenTarget;

    fn test_envelope(from: ScreenId, to: ScreenTarget) -> ScreenEnvelope {
        ScreenEnvelope {
            from,
            to,
            event_type: "test".to_owned(),
            payload: serde_json::json!({ "v": 1 }),
        }
    }

    #[tokio::test]
    async fn register_and_receive() {
        let registry = ScreenRegistry::new();
        let handle = registry.register(ScreenId::FrontScreen).await.unwrap();
        let (mut rx, _guard) = handle.into_parts();

        let envelope = test_envelope(
            ScreenId::BackScreen,
            ScreenTarget::Screen {
                id: ScreenId::FrontScreen,
            },
        );

        let sent = registry
            .send_to(ScreenId::FrontScreen, &envelope)
            .await
            .unwrap();
        assert!(sent);

        let received = rx.try_recv().unwrap();
        assert_eq!(received.event_type, "test");
    }

    #[tokio::test]
    async fn duplicate_register_fails() {
        let registry = ScreenRegistry::new();
        let handle = registry.register(ScreenId::FrontScreen).await.unwrap();
        let (_rx, _guard) = handle.into_parts();

        let result = registry.register(ScreenId::FrontScreen).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn send_to_disconnected_returns_false() {
        let registry = ScreenRegistry::new();

        let envelope = test_envelope(
            ScreenId::FrontScreen,
            ScreenTarget::Screen {
                id: ScreenId::BackScreen,
            },
        );

        let sent = registry
            .send_to(ScreenId::BackScreen, &envelope)
            .await
            .unwrap();
        assert!(!sent);
    }

    #[tokio::test]
    async fn broadcast_excludes_sender() {
        let registry = ScreenRegistry::new();

        let (mut front_rx, _fg) = registry
            .register(ScreenId::FrontScreen)
            .await
            .unwrap()
            .into_parts();
        let (mut back_rx, _bg) = registry
            .register(ScreenId::BackScreen)
            .await
            .unwrap()
            .into_parts();
        let (mut dmd_rx, _dg) = registry
            .register(ScreenId::DmdScreen)
            .await
            .unwrap()
            .into_parts();

        let envelope = test_envelope(ScreenId::FrontScreen, ScreenTarget::Broadcast);
        registry.broadcast(&envelope, ScreenId::FrontScreen).await;

        // front should NOT receive (it's the sender)
        assert!(front_rx.try_recv().is_err());

        // back and dmd should receive
        assert!(back_rx.try_recv().is_ok());
        assert!(dmd_rx.try_recv().is_ok());
    }

    #[tokio::test]
    async fn connected_screens_reflects_state() {
        let registry = ScreenRegistry::new();
        assert!(registry.connected_screens().await.is_empty());

        let handle = registry.register(ScreenId::DmdScreen).await.unwrap();
        let (_rx, guard) = handle.into_parts();
        assert_eq!(registry.connected_screens().await.len(), 1);
        assert!(registry.is_connected(ScreenId::DmdScreen).await);

        drop(guard);
        // Give the spawned cleanup task a moment.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        assert!(!registry.is_connected(ScreenId::DmdScreen).await);
    }
}
