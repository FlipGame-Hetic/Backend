use thiserror::Error;

#[derive(Debug, Error)]
pub enum BridgeError {
    #[error("MQTT client error: {0}")]
    Client(#[from] rumqttc::ClientError),

    #[error("MQTT connection error: {0}")]
    Connection(#[from] rumqttc::ConnectionError),

    #[error("Json serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Invalid topic: {0}")]
    Topic(#[from] shared::dto::TopicError),

    #[error("Websocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),

    #[error("Missing environment variable: {0}")]
    Config(String),
}

pub type Result<T> = std::result::Result<T, BridgeError>;
