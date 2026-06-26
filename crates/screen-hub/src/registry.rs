//! Connection registry for pinball screens.
//!
//! [`ScreenRegistry`] tracks which screens are currently connected by holding
//! the sender half of a per-screen mpsc channel.  Callers receive the receiver
//! half wrapped in a [`ScreenHandle`] and hold a [`ScreenGuard`] whose `Drop`
//! impl automatically cleans up the registry entry when the WebSocket session
//! ends.
use std::collections::HashMap;
use std::sync::Arc;

use shared::screen::{ScreenEnvelope, ScreenId};
use tokio::sync::{RwLock, mpsc};
use tracing::{debug, info, warn};

use crate::error::{Result, ScreenHubError};

/// Depth of the per-screen outbound channel.  128 messages of headroom before
/// the sender starts dropping frames for a slow screen.
const SCREEN_CHANNEL_CAPACITY: usize = 128;

/// RAII guard that automatically unregisters a screen when dropped.
///
/// Hold this for the entire lifetime of the WebSocket connection.  When the
/// connection closes and the guard falls out of scope, it spawns a one-shot
/// async task to remove the screen from the registry necessary because `Drop`
/// is synchronous and cannot `.await` directly.
pub struct ScreenGuard {
    screen_id: ScreenId,
    registry: Arc<ScreenRegistryInner>,
}

impl Drop for ScreenGuard {
    fn drop(&mut self) {
        let inner = Arc::clone(&self.registry);
        let id = self.screen_id;

        // `Drop` is sync, so we spawn a fire-and-forget task for the async remove.
        tokio::spawn(async move {
            inner.remove(id).await;
        });
    }
}

/// Handle returned when a screen successfully registers.
///
/// Split with [`into_parts`][Self::into_parts] to obtain:
/// - the **receiver** — pass to the WS write loop to forward messages to the screen
/// - the **guard** — keep alive for the duration of the connection
pub struct ScreenHandle {
    rx: mpsc::Receiver<ScreenEnvelope>,
    guard: ScreenGuard,
}

impl ScreenHandle {
    /// Decompose into the message receiver and the cleanup guard.
    ///
    /// The [`ScreenGuard`] **must** remain alive until the connection ends;
    /// dropping it earlier unregisters the screen prematurely.
    pub fn into_parts(self) -> (mpsc::Receiver<ScreenEnvelope>, ScreenGuard) {
        (self.rx, self.guard)
    }
}

/// Shared mutable state behind the registry's `Arc`.
#[derive(Default)]
struct ScreenRegistryInner {
    /// Maps each connected screen to the sender half of its outbound channel.
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
/// Each screen gets a bounded mpsc channel when it registers.  This struct
/// holds the `Sender` halves; the screen's WS handler holds the `Receiver`
/// via the [`ScreenHandle`] returned by [`register`][Self::register].
///
/// **Separation of concerns**: this struct handles connection lifecycle only.
/// Routing logic (unicast, broadcast, interceptors) lives in
/// [`crate::router::ScreenRouter`].
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

    /// Register a screen and return a handle that delivers inbound messages.
    ///
    /// Fails with [`ScreenHubError::AlreadyConnected`] if a session for this
    /// screen is already active — callers should close the old connection first.
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

    /// Send an envelope to a specific screen using a non-blocking `try_send`.
    ///
    /// Returns:
    /// - `Ok(true)` message delivered to the channel
    /// - `Ok(false)` screen not connected or channel full (message dropped)
    /// - `Err(SendFailed)` channel was closed; stale entry removed automatically
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
                // We must release the read lock before calling `remove`, which
                // acquires a write lock.  Holding both would deadlock.
                drop(senders);
                self.inner.remove(id).await;
                Err(ScreenHubError::SendFailed(id))
            }
        }
    }

    /// Send an envelope to every connected screen except `exclude`.
    ///
    /// Uses `try_send` so a slow screen cannot block the others.  Closed
    /// channels are not cleaned up here — that is handled lazily by the next
    /// [`send_to`][Self::send_to] call or by the [`ScreenGuard`] drop.
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
                    // The ScreenGuard drop will clean this entry up asynchronously;
                    // no action needed here.
                    warn!(screen = %id, "screen channel closed during broadcast");
                }
            }
        }
    }

    /// Return a snapshot of currently connected screen IDs.
    pub async fn connected_screens(&self) -> Vec<ScreenId> {
        self.inner.senders.read().await.keys().copied().collect()
    }

    /// Return `true` if the given screen is currently registered.
    pub async fn is_connected(&self, id: ScreenId) -> bool {
        self.inner.senders.read().await.contains_key(&id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::screen::{ScreenEventType, ScreenTarget};

    fn test_envelope(from: ScreenId, to: ScreenTarget) -> ScreenEnvelope {
        ScreenEnvelope {
            from,
            to,
            event_type: ScreenEventType::Unknown("test".to_owned()),
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
        assert_eq!(
            received.event_type,
            ScreenEventType::Unknown("test".to_owned())
        );
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

        // The sender should NOT receive its own broadcast
        assert!(front_rx.try_recv().is_err());

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
        // The cleanup task spawned by ScreenGuard::drop is async; give it a
        // moment to run before asserting the screen is gone.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        assert!(!registry.is_connected(ScreenId::DmdScreen).await);
    }
}
