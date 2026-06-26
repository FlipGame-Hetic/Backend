//! Error types used throughout the `mqtt-bridge` crate.
//!
//! [`BridgeError`] covers every failure mode the relay can encounter:
//! MQTT protocol errors, WebSocket transport errors, JSON (de)serialisation
//! failures, bad topic strings, and missing or unparseable environment
//! variables.
//!
//! `ConnectionError` and `tungstenite::Error` are **boxed** to avoid bloating
//! the enum's stack size — both types are significantly larger than the other
//! variants.
use thiserror::Error;

/// All errors that can occur inside the mqtt-bridge.
#[derive(Debug, Error)]
pub enum BridgeError {
    /// The `rumqttc` async client itself failed (e.g. channel full, QoS limit
    /// exceeded).  Distinct from a *connection* error — the TCP session may
    /// still be alive.
    #[error("MQTT client error: {0}")]
    Client(#[from] rumqttc::ClientError),

    /// A broken or refused MQTT connection at the transport level.  Boxed
    /// because `rumqttc::ConnectionError` is a large enum.
    #[error("MQTT connection error: {0}")]
    Connection(Box<rumqttc::ConnectionError>),

    /// JSON serialisation or deserialisation failed for an MQTT payload or
    /// WebSocket frame.
    #[error("JSON serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// The MQTT topic string could not be parsed into a [`shared::dto::Topic`]
    /// (unknown prefix, missing device-id segment, unrecognised subtopic, etc.)
    #[error("invalid topic: {0}")]
    Topic(#[from] shared::dto::TopicError),

    /// The WebSocket connection to the central API failed.  Boxed for the same
    /// size reason as `Connection`.
    #[error("WebSocket error: {0}")]
    WebSocket(Box<tokio_tungstenite::tungstenite::Error>),

    /// A required environment variable is missing or contains an invalid value.
    /// The inner `String` names the variable and describes the constraint.
    #[error("missing environment variable: {0}")]
    Config(String),
}

// Manual From impls because deriving would embed the value inline and make
// BridgeError noticeably larger on the stack for the common cases.
impl From<rumqttc::ConnectionError> for BridgeError {
    fn from(err: rumqttc::ConnectionError) -> Self {
        Self::Connection(Box::new(err))
    }
}

impl From<tokio_tungstenite::tungstenite::Error> for BridgeError {
    fn from(err: tokio_tungstenite::tungstenite::Error) -> Self {
        Self::WebSocket(Box::new(err))
    }
}

/// Convenience alias so callers don't have to repeat the full `Result` type.
pub type Result<T> = std::result::Result<T, BridgeError>;
