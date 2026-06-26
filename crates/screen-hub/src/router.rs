//! Message routing layer on top of [`ScreenRegistry`].
//!
//! [`ScreenRouter`] is the single entry point for dispatching a
//! [`ScreenEnvelope`] to its intended target(s).  Before delivery it runs the
//! envelope through a configurable chain of [`Interceptor`]s, any of which can
//! mutate or swallow the message.
use shared::screen::{ScreenEnvelope, ScreenId, ScreenTarget};
use tracing::{debug, warn};

use crate::error::ScreenHubError;
use crate::registry::ScreenRegistry;

/// Routes [`ScreenEnvelope`] messages to the correct screens via the registry.
///
/// This is the single entry point for dispatching screen-to-screen messages.
/// The interceptor pipeline runs first; only if all interceptors pass does the
/// message reach the registry.  New processing steps (e.g. score enrichment,
/// combo detection, validation) can be added as interceptors without touching
/// the WS handler or the registry.
pub struct ScreenRouter {
    registry: ScreenRegistry,
    /// Ordered list of interceptors.  Each one sees the (possibly mutated)
    /// envelope from the previous step; returning `None` short-circuits the
    /// rest of the chain and prevents delivery.
    interceptors: Vec<Box<dyn Interceptor>>,
}

/// Hook point for preprocessing messages before they are dispatched.
///
/// Interceptors run **before** delivery, in registration order.  They can:
/// - Inspect or log the envelope
/// - Mutate fields (e.g. enrich the payload)
/// - Return `None` to swallow the message (e.g. failed validation)
///
/// If any interceptor returns `None`, the remaining interceptors are skipped
/// and the message is never delivered.
pub trait Interceptor: Send + Sync {
    fn process(&self, envelope: ScreenEnvelope) -> Option<ScreenEnvelope>;
}

/// Summary of what happened during a [`ScreenRouter::dispatch`] call.
#[derive(Debug)]
pub struct DispatchResult {
    /// Number of screens the message was successfully enqueued to.
    pub delivered: usize,
    /// Screens that were targeted but not currently connected.
    pub missed: Vec<ScreenId>,
    /// `true` if an interceptor returned `None` and prevented delivery.
    pub intercepted: bool,
}

impl ScreenRouter {
    pub fn new(registry: ScreenRegistry) -> Self {
        Self {
            registry,
            interceptors: Vec::new(),
        }
    }

    /// Append an interceptor to the end of the processing pipeline.
    ///
    /// Interceptors are invoked in the order they are added.
    pub fn add_interceptor(&mut self, interceptor: Box<dyn Interceptor>) {
        self.interceptors.push(interceptor);
    }

    /// Route an envelope to its target(s) after running the interceptor chain.
    ///
    /// Returns a [`DispatchResult`] describing delivery outcome regardless of
    /// whether the envelope was intercepted, missed, or delivered.
    pub async fn dispatch(&self, envelope: ScreenEnvelope) -> DispatchResult {
        let envelope = match self.run_interceptors(envelope) {
            Some(e) => e,
            None => {
                // An interceptor returned None; stop here without dispatching.
                debug!(event_type = %"intercepted", "message swallowed by interceptor");
                return DispatchResult {
                    delivered: 0,
                    missed: Vec::new(),
                    intercepted: true,
                };
            }
        };

        match &envelope.to {
            ScreenTarget::Screen { id } => self.dispatch_to_one(*id, &envelope).await,
            ScreenTarget::Broadcast => self.dispatch_broadcast(&envelope).await,
        }
    }

    /// Run the envelope through every interceptor in order.
    ///
    /// Uses the `?` operator on `Option` to short-circuit as soon as one
    /// interceptor returns `None`, skipping all subsequent interceptors.
    fn run_interceptors(&self, mut envelope: ScreenEnvelope) -> Option<ScreenEnvelope> {
        for interceptor in &self.interceptors {
            envelope = interceptor.process(envelope)?;
        }
        Some(envelope)
    }

    /// Deliver a message to exactly one target screen.
    async fn dispatch_to_one(&self, target: ScreenId, envelope: &ScreenEnvelope) -> DispatchResult {
        match self.registry.send_to(target, envelope).await {
            Ok(true) => {
                debug!(
                    from = %envelope.from,
                    to = %target,
                    event_type = %envelope.event_type,
                    "dispatched to screen"
                );
                DispatchResult {
                    delivered: 1,
                    missed: Vec::new(),
                    intercepted: false,
                }
            }
            Ok(false) => {
                warn!(
                    from = %envelope.from,
                    to = %target,
                    event_type = %envelope.event_type,
                    "target screen not connected"
                );
                DispatchResult {
                    delivered: 0,
                    missed: vec![target],
                    intercepted: false,
                }
            }
            Err(ScreenHubError::SendFailed(id)) => {
                warn!(screen = %id, "send failed, channel closed");
                DispatchResult {
                    delivered: 0,
                    missed: vec![id],
                    intercepted: false,
                }
            }
            Err(e) => {
                warn!(error = %e, "unexpected dispatch error");
                DispatchResult {
                    delivered: 0,
                    missed: vec![target],
                    intercepted: false,
                }
            }
        }
    }

    /// Broadcast a message to all connected screens except the sender.
    ///
    /// `delivered` is derived from [`ScreenRegistry::connected_screens`]
    /// **after** the broadcast, so it may be slightly off if a screen connects
    /// or disconnects in the window between the two calls — treat it as
    /// approximate.
    async fn dispatch_broadcast(&self, envelope: &ScreenEnvelope) -> DispatchResult {
        let sender = envelope.from;

        self.registry.broadcast(envelope, sender).await;

        // Snapshot taken after broadcast to count recipients; see doc note above.
        let connected = self.registry.connected_screens().await;
        let delivered = connected.iter().filter(|&&id| id != sender).count();

        debug!(
            from = %sender,
            event_type = %envelope.event_type,
            delivered,
            "broadcast dispatched"
        );

        DispatchResult {
            delivered,
            missed: Vec::new(),
            intercepted: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::screen::{ScreenEnvelope, ScreenEventType};

    fn envelope(from: ScreenId, to: ScreenTarget) -> ScreenEnvelope {
        ScreenEnvelope {
            from,
            to,
            event_type: ScreenEventType::Unknown("test_event".to_owned()),
            payload: serde_json::json!({ "data": "hello" }),
        }
    }

    /// Interceptor that passes everything through unchanged.
    struct PassThrough;
    impl Interceptor for PassThrough {
        fn process(&self, envelope: ScreenEnvelope) -> Option<ScreenEnvelope> {
            Some(envelope)
        }
    }

    /// Interceptor that swallows all messages.
    struct Swallow;
    impl Interceptor for Swallow {
        fn process(&self, _envelope: ScreenEnvelope) -> Option<ScreenEnvelope> {
            None
        }
    }

    /// Interceptor that prepends `"mutated_"` to the event type string.
    struct Mutator;
    impl Interceptor for Mutator {
        fn process(&self, mut envelope: ScreenEnvelope) -> Option<ScreenEnvelope> {
            envelope.event_type =
                ScreenEventType::Unknown(format!("mutated_{}", envelope.event_type));
            Some(envelope)
        }
    }

    #[tokio::test]
    async fn dispatch_to_specific_screen() {
        let registry = ScreenRegistry::new();
        let router = ScreenRouter::new(registry.clone());

        let (mut rx, _guard) = registry
            .register(ScreenId::BackScreen)
            .await
            .unwrap()
            .into_parts();

        let env = envelope(
            ScreenId::FrontScreen,
            ScreenTarget::Screen {
                id: ScreenId::BackScreen,
            },
        );

        let result = router.dispatch(env).await;
        assert_eq!(result.delivered, 1);
        assert!(result.missed.is_empty());
        assert!(!result.intercepted);

        let msg = rx.try_recv().unwrap();
        assert_eq!(
            msg.event_type,
            ScreenEventType::Unknown("test_event".to_owned())
        );
    }

    #[tokio::test]
    async fn dispatch_to_disconnected_screen() {
        let registry = ScreenRegistry::new();
        let router = ScreenRouter::new(registry.clone());

        let env = envelope(
            ScreenId::FrontScreen,
            ScreenTarget::Screen {
                id: ScreenId::DmdScreen,
            },
        );

        let result = router.dispatch(env).await;
        assert_eq!(result.delivered, 0);
        assert_eq!(result.missed, vec![ScreenId::DmdScreen]);
    }

    #[tokio::test]
    async fn broadcast_delivers_to_all_except_sender() {
        let registry = ScreenRegistry::new();
        let router = ScreenRouter::new(registry.clone());

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
        let (_dmd_rx, _dg) = registry
            .register(ScreenId::DmdScreen)
            .await
            .unwrap()
            .into_parts();

        let env = envelope(ScreenId::DmdScreen, ScreenTarget::Broadcast);

        let result = router.dispatch(env).await;
        assert_eq!(result.delivered, 2);

        assert!(front_rx.try_recv().is_ok());
        assert!(back_rx.try_recv().is_ok());
    }

    #[tokio::test]
    async fn interceptor_swallows_message() {
        let registry = ScreenRegistry::new();
        let mut router = ScreenRouter::new(registry.clone());
        router.add_interceptor(Box::new(Swallow));

        let (mut rx, _guard) = registry
            .register(ScreenId::BackScreen)
            .await
            .unwrap()
            .into_parts();

        let env = envelope(
            ScreenId::FrontScreen,
            ScreenTarget::Screen {
                id: ScreenId::BackScreen,
            },
        );

        let result = router.dispatch(env).await;
        assert!(result.intercepted);
        assert_eq!(result.delivered, 0);

        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn interceptor_mutates_message() {
        let registry = ScreenRegistry::new();
        let mut router = ScreenRouter::new(registry.clone());
        router.add_interceptor(Box::new(Mutator));

        let (mut rx, _guard) = registry
            .register(ScreenId::BackScreen)
            .await
            .unwrap()
            .into_parts();

        let env = envelope(
            ScreenId::FrontScreen,
            ScreenTarget::Screen {
                id: ScreenId::BackScreen,
            },
        );

        router.dispatch(env).await;

        let msg = rx.try_recv().unwrap();
        assert_eq!(
            msg.event_type,
            ScreenEventType::Unknown("mutated_test_event".to_owned())
        );
    }

    #[tokio::test]
    async fn interceptors_chain_in_order() {
        let registry = ScreenRegistry::new();
        let mut router = ScreenRouter::new(registry.clone());
        router.add_interceptor(Box::new(PassThrough));
        router.add_interceptor(Box::new(Mutator));

        let (mut rx, _guard) = registry
            .register(ScreenId::BackScreen)
            .await
            .unwrap()
            .into_parts();

        let env = envelope(
            ScreenId::FrontScreen,
            ScreenTarget::Screen {
                id: ScreenId::BackScreen,
            },
        );

        router.dispatch(env).await;

        let msg = rx.try_recv().unwrap();
        assert_eq!(
            msg.event_type,
            ScreenEventType::Unknown("mutated_test_event".to_owned())
        );
    }

    #[tokio::test]
    async fn interceptor_swallow_stops_chain() {
        let registry = ScreenRegistry::new();
        let mut router = ScreenRouter::new(registry.clone());
        router.add_interceptor(Box::new(Swallow));
        router.add_interceptor(Box::new(Mutator)); // must never run

        let (mut rx, _guard) = registry
            .register(ScreenId::BackScreen)
            .await
            .unwrap()
            .into_parts();

        let env = envelope(
            ScreenId::FrontScreen,
            ScreenTarget::Screen {
                id: ScreenId::BackScreen,
            },
        );

        let result = router.dispatch(env).await;
        assert!(result.intercepted);
        assert!(rx.try_recv().is_err());
    }
}
