//! Error types for the `screen-hub` crate.
//!
//! [`ScreenHubError`] covers the four failure modes that can occur during
//! screen registration or message dispatch.
use shared::screen::ScreenId;
use thiserror::Error;

/// All errors that can occur inside `screen-hub`.
#[derive(Debug, Error)]
pub enum ScreenHubError {
    /// A [`ScreenRegistry::register`][crate::registry::ScreenRegistry::register]
    /// call was made for a screen that is already present in the registry.
    /// Only one WebSocket session per screen is allowed at a time.
    #[error("screen already connected: {0}")]
    AlreadyConnected(ScreenId),

    /// A targeted send was attempted for a screen that has no entry in the
    /// registry (never connected, or already unregistered).
    #[error("screen not connected: {0}")]
    NotConnected(ScreenId),

    /// The mpsc channel for the given screen was closed while a send was in
    /// progress the screen disconnected without going through the normal
    /// unregistration path.  The stale entry is cleaned up automatically on
    /// this error.
    #[error("failed to send message to {0}: channel closed")]
    SendFailed(ScreenId),

    /// JSON serialisation or deserialisation failed for a screen envelope
    /// payload.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// Convenience alias so callers don't have to repeat the full `Result` type.
pub type Result<T> = std::result::Result<T, ScreenHubError>;
