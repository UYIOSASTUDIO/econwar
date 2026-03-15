//! Middleware: JWT auth extraction, rate limiting, etc.
//!
//! For MVP, auth is simplified.  The full middleware stack will include:
//!   - JWT verification (extract player_id from Authorization header)
//!   - Rate limiting per player
//!   - Request logging

use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
};
use jsonwebtoken::{decode, DecodingKey, Validation};
use uuid::Uuid;

use crate::api::auth::Claims;

/// Extractor that pulls the authenticated player ID from the JWT.
pub struct AuthPlayer {
    pub player_id: Uuid,
    pub username: String,
}

fn jwt_secret() -> Vec<u8> {
    std::env::var("JWT_SECRET")
        .unwrap_or_else(|_| "econwar-dev-secret-change-me".into())
        .into_bytes()
}

impl<S: Send + Sync> FromRequestParts<S> for AuthPlayer {
    type Rejection = (StatusCode, String);

    fn from_request_parts<'life0, 'life1, 'async_trait>(
        parts: &'life0 mut Parts,
        _state: &'life1 S,
    ) -> core::pin::Pin<Box<dyn core::future::Future<Output = Result<Self, Self::Rejection>> + Send + 'async_trait>>
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(async move {
            let auth_header = parts
                .headers
                .get("Authorization")
                .and_then(|v| v.to_str().ok())
                .ok_or((StatusCode::UNAUTHORIZED, "Missing Authorization header".into()))?;

            let token = auth_header
                .strip_prefix("Bearer ")
                .ok_or((StatusCode::UNAUTHORIZED, "Invalid Authorization format".into()))?;

            let token_data = decode::<Claims>(
                token,
                &DecodingKey::from_secret(&jwt_secret()),
                &Validation::default(),
            )
            .map_err(|e| (StatusCode::UNAUTHORIZED, format!("Invalid token: {e}")))?;

            Ok(AuthPlayer {
                player_id: token_data.claims.sub,
                username: token_data.claims.username,
            })
        })
    }
}
