use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use jsonwebtoken::{Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};

use crate::errors::ApiError;
use crate::state::AppState;

const ADMIN_ROLE: &str = "admin";

#[derive(Debug, Serialize, Deserialize)]
pub struct AdminClaims {
    pub sub: String,
    pub role: String,
}

/// Axum extractor that validates an `Authorization: Bearer <jwt>` header.
///
/// The token must be signed with HS256 using `SCREEN_JWT_SECRET` and carry
/// `role = "admin"`. Requests that fail validation receive a 401.
///
/// Generate a token once with (run in the workspace root):
///
/// ```text
/// cargo test -p api admin::auth::tests::print_admin_token -- --nocapture
/// ```
pub struct AdminUser;

impl FromRequestParts<AppState> for AdminUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let bearer = extract_bearer(parts)?;
        verify_admin_token(bearer, &state.jwt_secret)?;
        Ok(AdminUser)
    }
}

fn extract_bearer<'a>(parts: &'a Parts) -> Result<&'a str, ApiError> {
    parts
        .headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| ApiError::Unauthorized("missing Authorization: Bearer header".to_owned()))
}

fn verify_admin_token(token: &str, secret: &[u8]) -> Result<AdminClaims, ApiError> {
    let key = DecodingKey::from_secret(secret);
    let mut validation = Validation::new(Algorithm::HS256);
    validation.required_spec_claims.clear();
    validation.validate_exp = false;

    let data = jsonwebtoken::decode::<AdminClaims>(token, &key, &validation)
        .map_err(|_| ApiError::Unauthorized("invalid admin token".to_owned()))?;

    if data.claims.role != ADMIN_ROLE {
        return Err(ApiError::Unauthorized("token role is not admin".to_owned()));
    }

    Ok(data.claims)
}

#[cfg(test)]
pub fn generate_admin_token(secret: &[u8]) -> String {
    let claims = AdminClaims {
        sub: "admin".to_owned(),
        role: ADMIN_ROLE.to_owned(),
    };
    let key = jsonwebtoken::EncodingKey::from_secret(secret);
    let header = jsonwebtoken::Header::new(Algorithm::HS256);
    jsonwebtoken::encode(&header, &claims, &key).expect("token generation failed")
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_SECRET: &[u8] = b"test-secret-key-for-unit-tests-32chars!";

    #[test]
    fn valid_admin_token_is_accepted() {
        let token = generate_admin_token(TEST_SECRET);
        let claims = verify_admin_token(&token, TEST_SECRET).unwrap();
        assert_eq!(claims.role, "admin");
    }

    #[test]
    fn wrong_secret_is_rejected() {
        let token = generate_admin_token(TEST_SECRET);
        assert!(verify_admin_token(&token, b"other-secret-32-chars-loooooong!").is_err());
    }

    #[test]
    fn wrong_role_is_rejected() {
        let claims = AdminClaims {
            sub: "user".to_owned(),
            role: "viewer".to_owned(),
        };
        let key = jsonwebtoken::EncodingKey::from_secret(TEST_SECRET);
        let token =
            jsonwebtoken::encode(&jsonwebtoken::Header::new(Algorithm::HS256), &claims, &key)
                .unwrap();
        assert!(verify_admin_token(&token, TEST_SECRET).is_err());
    }

    #[test]
    fn garbage_token_is_rejected() {
        assert!(verify_admin_token("not.a.jwt", TEST_SECRET).is_err());
    }

    /// Run with: cargo test -p api admin::auth::tests::print_admin_token -- --nocapture
    ///
    /// Prints the admin JWT token. Set SCREEN_JWT_SECRET env var to use the prod secret.
    #[test]
    fn print_admin_token() {
        let secret = std::env::var("SCREEN_JWT_SECRET")
            .unwrap_or_else(|_| "flipper-dev-secret-change-in-prod".to_owned());
        let token = generate_admin_token(secret.as_bytes());
        println!("\n# Admin JWT token (add to Authorization: Bearer header)");
        println!("ADMIN_TOKEN={token}\n");
    }
}
