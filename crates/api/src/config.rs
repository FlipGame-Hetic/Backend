use std::net::SocketAddr;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("invalid config: {0}")]
    Invalid(String),
}

#[derive(Debug, Clone)]
pub struct ApiConfig {
    pub port: u16,
    pub allowed_origins: Vec<String>,
    pub jwt_secret: String,
}

impl ApiConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        let port = std::env::var("API_PORT")
            .unwrap_or_else(|_| "8080".to_owned())
            .parse::<u16>()
            .map_err(|_| ConfigError::Invalid("API_PORT must be a valid u16".to_owned()))?;

        let allowed_origins = std::env::var("ALLOWED_ORIGINS")
            .unwrap_or_else(|_| "http://localhost:3000".to_owned())
            .split(',')
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .collect();

        let jwt_secret = std::env::var("SCREEN_JWT_SECRET")
            .unwrap_or_else(|_| "flipper-dev-secret-change-in-prod".to_owned());

        if jwt_secret.len() < 32 {
            return Err(ConfigError::Invalid(
                "SCREEN_JWT_SECRET must be at least 32 characters".to_owned(),
            ));
        }

        Ok(Self {
            port,
            allowed_origins,
            jwt_secret,
        })
    }

    pub fn socket_addr(&self) -> SocketAddr {
        SocketAddr::from(([0, 0, 0, 0], self.port))
    }
}
