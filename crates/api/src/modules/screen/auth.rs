use jsonwebtoken::{Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use shared::screen::ScreenId;
use tracing::warn;

/// JWT claims embedded in each screen token.
///
/// Tokens are pre-generated and stored in frontend `.env` files.
/// The backend validates them on WebSocket upgrade to identify which
/// screen is connecting. There is no expiration by design — these are
/// device-identity tokens, not user-session tokens.
#[derive(Debug, Serialize, Deserialize)]
pub struct ScreenClaims {
    /// The screen this token identifies.
    pub screen_id: ScreenId,

    /// Standard JWT subject — set to the screen id string for convenience.
    pub sub: String,
}

/// Errors that can occur during screen token verification.
#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("invalid token: {0}")]
    InvalidToken(#[from] jsonwebtoken::errors::Error),

    #[error("screen_id mismatch: token claims '{claimed}' but route says '{expected}'")]
    ScreenMismatch {
        claimed: ScreenId,
        expected: ScreenId,
    },
}

/// Verify a screen JWT and return the claimed `ScreenId`.
///
/// The token is validated using HMAC-SHA256 with the shared secret.
/// No expiration check — these tokens are quasi-permanent.
pub fn verify_screen_token(token: &str, secret: &[u8]) -> Result<ScreenClaims, AuthError> {
    let key = DecodingKey::from_secret(secret);

    let mut validation = Validation::new(Algorithm::HS256);
    validation.required_spec_claims.clear();
    validation.validate_exp = false;

    let token_data = jsonwebtoken::decode::<ScreenClaims>(token, &key, &validation)?;

    Ok(token_data.claims)
}

/// Verify a screen JWT **and** check that the claimed `screen_id` matches
/// the one from the URL path. This prevents a `front_screen` token from
/// connecting to the `/ws/screen/back_screen` endpoint.
pub fn verify_and_match(
    token: &str,
    secret: &[u8],
    expected: ScreenId,
) -> Result<ScreenClaims, AuthError> {
    let claims = verify_screen_token(token, secret)?;

    if claims.screen_id != expected {
        warn!(
            claimed = %claims.screen_id,
            expected = %expected,
            "screen_id mismatch in JWT"
        );
        return Err(AuthError::ScreenMismatch {
            claimed: claims.screen_id,
            expected,
        });
    }

    Ok(claims)
}

/// Generate a screen token (utility for tests and initial setup).
///
/// In production, tokens are generated once and stored in `.env` files.
/// Also used by the `generate-tokens` binary to create initial `.env` values.
#[cfg(test)]
pub fn generate_screen_token(screen_id: ScreenId, secret: &[u8]) -> Result<String, AuthError> {
    let claims = ScreenClaims {
        sub: screen_id.to_string(),
        screen_id,
    };

    let key = jsonwebtoken::EncodingKey::from_secret(secret);
    let header = jsonwebtoken::Header::new(Algorithm::HS256);
    let token = jsonwebtoken::encode(&header, &claims, &key)?;

    Ok(token)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_SECRET: &[u8] = b"test-secret-key-for-unit-tests";

    #[test]
    fn generate_and_verify_roundtrip() {
        let token = generate_screen_token(ScreenId::FrontScreen, TEST_SECRET).unwrap();
        let claims = verify_screen_token(&token, TEST_SECRET).unwrap();

        assert_eq!(claims.screen_id, ScreenId::FrontScreen);
        assert_eq!(claims.sub, "front_screen");
    }

    #[test]
    fn verify_and_match_succeeds_when_matching() {
        let token = generate_screen_token(ScreenId::BackScreen, TEST_SECRET).unwrap();
        let claims = verify_and_match(&token, TEST_SECRET, ScreenId::BackScreen).unwrap();

        assert_eq!(claims.screen_id, ScreenId::BackScreen);
    }

    #[test]
    fn verify_and_match_rejects_mismatch() {
        let token = generate_screen_token(ScreenId::FrontScreen, TEST_SECRET).unwrap();
        let result = verify_and_match(&token, TEST_SECRET, ScreenId::BackScreen);

        assert!(matches!(result, Err(AuthError::ScreenMismatch { .. })));
    }

    #[test]
    fn reject_wrong_secret() {
        let token = generate_screen_token(ScreenId::DmdScreen, TEST_SECRET).unwrap();
        let result = verify_screen_token(&token, b"wrong-secret");

        assert!(matches!(result, Err(AuthError::InvalidToken(_))));
    }

    #[test]
    fn reject_garbage_token() {
        let result = verify_screen_token("not.a.jwt", TEST_SECRET);
        assert!(result.is_err());
    }

    #[test]
    fn all_screen_ids_roundtrip() {
        for &id in ScreenId::all() {
            let token = generate_screen_token(id, TEST_SECRET).unwrap();
            let claims = verify_and_match(&token, TEST_SECRET, id).unwrap();
            assert_eq!(claims.screen_id, id);
        }
    }

    /// Run with: cargo test -p api print_all_screen_tokens -- --nocapture
    ///
    /// Prints JWT tokens for all screens, ready to paste into frontend `.env` files.
    /// Uses the dev secret by default — set SCREEN_JWT_SECRET env var to override.
    #[test]
    fn print_all_screen_tokens() {
        let secret = std::env::var("SCREEN_JWT_SECRET")
            .unwrap_or_else(|_| "flipper-dev-secret-change-in-prod".to_owned());

        println!("\n# Screen JWT tokens (paste into frontend .env files)");

        for &id in ScreenId::all() {
            let token = generate_screen_token(id, secret.as_bytes()).unwrap();
            println!("# {id}");
            println!("VITE_SCREEN_TOKEN={token}\n");
        }
    }
}
