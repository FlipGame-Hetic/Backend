use std::collections::HashMap;
use std::net::SocketAddr;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("invalid config: {0}")]
    Invalid(String),
}

/// Runtime configuration loaded from environment variables at startup
#[derive(Debug, Clone)]
pub struct ApiConfig {
    /// TCP port the HTTP server listens on defaults to 8080
    pub port: u16,
    /// CORS allowed origins; a single `"*"` entry opens CORS to any origin
    pub allowed_origins: Vec<String>,
    /// HMAC-SHA256 secret used for both admin JWTs and screen JWTs — must be ≥ 32 chars
    pub jwt_secret: String,
    /// SQLite connection URL, e.g. `sqlite:///data/flipper.db`
    pub database_url: String,
}

impl ApiConfig {
    /// Read config from the actual process environment
    pub fn from_env() -> Result<Self, ConfigError> {
        let vars: HashMap<String, String> = std::env::vars().collect();
        Self::from_map(&vars)
    }

    /// Pure constructor used by tests reads from an explicit map instead of the process env.
    pub fn from_map(vars: &HashMap<String, String>) -> Result<Self, ConfigError> {
        let port = vars
            .get("API_PORT")
            .map(|s| s.as_str())
            .unwrap_or("8080")
            .parse::<u16>()
            .map_err(|_| ConfigError::Invalid("API_PORT must be a valid u16".to_owned()))?;

        let allowed_origins = vars
            .get("ALLOWED_ORIGINS")
            .map(|s| s.as_str())
            .unwrap_or("http://localhost:3000")
            .split(',')
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .collect();

        let jwt_secret = vars.get("SCREEN_JWT_SECRET").cloned().ok_or_else(|| {
            ConfigError::Invalid("SCREEN_JWT_SECRET env var is required".to_owned())
        })?;

        // 32 chars is the minimum to satisfy HMAC-SHA256 key strength requirements
        if jwt_secret.len() < 32 {
            return Err(ConfigError::Invalid(
                "SCREEN_JWT_SECRET must be at least 32 characters".to_owned(),
            ));
        }

        let database_url = vars
            .get("DATABASE_URL")
            .cloned()
            .unwrap_or_else(|| "sqlite:///data/flipper.db".to_owned());

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

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert(
            "SCREEN_JWT_SECRET".to_owned(),
            "a-secret-that-is-long-enough-32chars!".to_owned(),
        );
        m
    }

    #[test]
    fn defaults_are_applied_when_only_secret_is_set() {
        let cfg = ApiConfig::from_map(&base()).unwrap();
        assert_eq!(cfg.port, 8080);
        assert_eq!(cfg.database_url, "sqlite:///data/flipper.db");
        assert_eq!(cfg.allowed_origins, vec!["http://localhost:3000"]);
    }

    #[test]
    fn database_url_is_overridden_by_env_var() {
        let mut vars = base();
        vars.insert(
            "DATABASE_URL".to_owned(),
            "sqlite:///custom/path.db".to_owned(),
        );
        let cfg = ApiConfig::from_map(&vars).unwrap();
        assert_eq!(cfg.database_url, "sqlite:///custom/path.db");
    }

    #[test]
    fn missing_jwt_secret_returns_error() {
        let err = ApiConfig::from_map(&HashMap::new()).unwrap_err();
        assert!(err.to_string().contains("SCREEN_JWT_SECRET"));
    }

    #[test]
    fn short_jwt_secret_returns_error() {
        let mut vars = HashMap::new();
        vars.insert("SCREEN_JWT_SECRET".to_owned(), "tooshort".to_owned());
        let err = ApiConfig::from_map(&vars).unwrap_err();
        assert!(err.to_string().contains("32 characters"));
    }

    #[test]
    fn invalid_port_returns_error() {
        let mut vars = base();
        vars.insert("API_PORT".to_owned(), "not_a_number".to_owned());
        let err = ApiConfig::from_map(&vars).unwrap_err();
        assert!(err.to_string().contains("API_PORT"));
    }

    #[test]
    fn multiple_allowed_origins_are_parsed() {
        let mut vars = base();
        vars.insert(
            "ALLOWED_ORIGINS".to_owned(),
            "http://localhost:3000, https://prod.example.com".to_owned(),
        );
        let cfg = ApiConfig::from_map(&vars).unwrap();
        assert_eq!(cfg.allowed_origins.len(), 2);
        assert!(
            cfg.allowed_origins
                .contains(&"https://prod.example.com".to_owned())
        );
    }

    #[test]
    fn socket_addr_uses_configured_port() {
        let mut vars = base();
        vars.insert("API_PORT".to_owned(), "9090".to_owned());
        let cfg = ApiConfig::from_map(&vars).unwrap();
        assert_eq!(cfg.socket_addr().port(), 9090);
    }
}
