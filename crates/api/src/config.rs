use std::net::SocketAddr;

use percent_encoding::{NON_ALPHANUMERIC, percent_encode};
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
    pub database_url: String,
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

        let jwt_secret = std::env::var("SCREEN_JWT_SECRET").map_err(|_| {
            ConfigError::Invalid("SCREEN_JWT_SECRET env var is required".to_owned())
        })?;

        if jwt_secret.len() < 32 {
            return Err(ConfigError::Invalid(
                "SCREEN_JWT_SECRET must be at least 32 characters".to_owned(),
            ));
        }

        let database_url = if let Ok(url) = std::env::var("DATABASE_URL") {
            url
        } else if let Ok(host) = std::env::var("DATABASE_HOST") {
            let port = std::env::var("DATABASE_PORT").unwrap_or_else(|_| "5432".to_owned());
            let name = std::env::var("DATABASE_NAME").unwrap_or_else(|_| "flipper".to_owned());
            let user = std::env::var("DATABASE_USER").unwrap_or_else(|_| "flipper".to_owned());
            let password = std::env::var("DATABASE_PASSWORD").unwrap_or_default();
            let user_enc = percent_encode(user.as_bytes(), NON_ALPHANUMERIC).to_string();
            let password_enc = percent_encode(password.as_bytes(), NON_ALPHANUMERIC).to_string();
            format!("postgresql://{user_enc}:{password_enc}@{host}:{port}/{name}")
        } else {
            if std::env::var("APP_ENV").unwrap_or_default() == "production" {
                return Err(ConfigError::Invalid(
                    "DATABASE_URL or DATABASE_HOST must be set in production".to_owned(),
                ));
            }
            tracing::warn!(
                "No DATABASE_URL or DATABASE_HOST set — falling back to local SQLite (dev only)"
            );
            "sqlite:///data/flipper.db".to_owned()
        };

        Ok(Self {
            port,
            allowed_origins,
            jwt_secret,
            database_url,
        })
    }

    pub fn socket_addr(&self) -> SocketAddr {
        SocketAddr::from(([0, 0, 0, 0], self.port))
    }
}
