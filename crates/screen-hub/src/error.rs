use shared::screen::ScreenId;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ScreenHubError {
    #[error("screen already connected: {0}")]
    AlreadyConnected(ScreenId),

    #[error("screen not connected: {0}")]
    NotConnected(ScreenId),

    #[error("failed to send message to {0}: channel closed")]
    SendFailed(ScreenId),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, ScreenHubError>;
