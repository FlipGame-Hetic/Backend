use thiserror::Error;

#[derive(Debug, Error)]
pub enum BridgeError {
    #[error("MQTT client error: {0}")]
    Client(#[from] rumqttc::ClientError),

    #[error("MQTT connection error: {0}")]
    Connection(Box<rumqttc::ConnectionError>),

    #[error("JSON serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("invalid topic: {0}")]
    Topic(#[from] shared::dto::TopicError),

    #[error("WebSocket error: {0}")]
    WebSocket(Box<tokio_tungstenite::tungstenite::Error>),

    #[error("missing environment variable: {0}")]
    Config(String),
}

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

pub type Result<T> = std::result::Result<T, BridgeError>;
