//! # Authentication utilities (JWT)
//!
//! This module provides **pure** JWT creation and decoding utilities.
//! It does **not** access environment variables directly.
//!
//! ## Design principles
//! - No dependency on `std::env`
//! - No global state
//! - Fully testable with deterministic inputs
//!
//! The JWT secret must be supplied by the caller (typically from `AppConfig`).
//!
//! ## Provided functions
//! - [`create_jwt`] — Create a signed JWT token
//! - [`decode_jwt`] — Validate and decode a JWT token

use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

/// JWT claims stored inside the token payload.
///
/// ## Fields
/// - `sub`: Subject (user ID)
/// - `exp`: Expiration time (UNIX timestamp, seconds)
///
/// This struct is serialized into the JWT payload.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Claims {
    /// Subject — typically the user ID
    pub sub: String,
    /// Expiration timestamp (UTC, seconds since UNIX epoch)
    pub exp: usize,
}

/// Creates a signed JWT for the given user ID.
///
/// ## Arguments
/// - `id`: User ID
/// - `secret`: HMAC secret used to sign the token
///
/// ## Returns
/// A signed JWT string.
///
/// ## Errors
/// Returns an error if:
/// - JWT encoding fails
///
/// ## Example
/// ```
/// use wzs_web::auth::jwt::create_jwt;
///
/// let secret = "test-secret";
/// let token = create_jwt(123, secret).unwrap();
/// assert!(!token.is_empty());
/// ```
pub fn create_jwt(id: u64, secret: &str) -> anyhow::Result<String> {
    let expiration = Utc::now()
        .checked_add_signed(Duration::hours(48))
        .expect("invalid timestamp")
        .timestamp() as usize;

    let claims = Claims {
        sub: id.to_string(),
        exp: expiration,
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )?;

    Ok(token)
}

/// Decodes and validates a JWT token.
///
/// ## Arguments
/// - `token`: JWT string
/// - `secret`: HMAC secret used to verify the signature
///
/// ## Returns
/// Decoded [`Claims`] if the token is valid.
///
/// ## Errors
/// Returns an error if:
/// - The token is malformed
/// - Signature does not match
/// - Token is expired
///
/// ## Example
/// ```
/// use wzs_web::auth::jwt::{create_jwt, decode_jwt};
///
/// let secret = "test-secret";
/// let token = create_jwt(1, secret).unwrap();
/// let claims = decode_jwt(&token, secret).unwrap();
///
/// assert_eq!(claims.sub, "1");
/// ```
pub fn decode_jwt(token: &str, secret: &str) -> anyhow::Result<Claims> {
    let decoded = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )?;

    Ok(decoded.claims)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "unit-test-secret";

    #[test]
    fn create_and_decode_roundtrip() {
        let token = create_jwt(42, SECRET).unwrap();
        let claims = decode_jwt(&token, SECRET).unwrap();

        assert_eq!(claims.sub, "42");
    }

    #[test]
    fn expiration_is_in_the_future() {
        let token = create_jwt(1, SECRET).unwrap();
        let claims = decode_jwt(&token, SECRET).unwrap();

        let now = Utc::now().timestamp() as usize;
        assert!(claims.exp > now, "expected expiration to be in the future");
    }

    #[test]
    fn invalid_signature_is_rejected() {
        let token = create_jwt(1, SECRET).unwrap();

        let wrong_secret = "wrong-secret";
        let result = decode_jwt(&token, wrong_secret);

        assert!(result.is_err());
    }

    #[test]
    fn malformed_token_is_rejected() {
        let result = decode_jwt("not-a-valid-token", SECRET);
        assert!(result.is_err());
    }
}
