//! Authentication endpoints: register and login.

use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use argon2::password_hash::SaltString;
use axum::{extract::State, http::StatusCode, Json};
use chrono::Utc;
use jsonwebtoken::{encode, EncodingKey, Header};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use econwar_db::repo;
use crate::state::SharedState;

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub player_id: Uuid,
    pub username: String,
}

/// JWT claims embedded in every token.
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: Uuid,       // player id
    pub username: String,
    pub exp: usize,      // expiration timestamp
}

/// Secret for signing JWTs.  In production, load from env/vault.
fn jwt_secret() -> Vec<u8> {
    std::env::var("JWT_SECRET")
        .unwrap_or_else(|_| "econwar-dev-secret-change-me".into())
        .into_bytes()
}

pub async fn register(
    State(state): State<SharedState>,
    Json(body): Json<RegisterRequest>,
) -> Result<Json<AuthResponse>, (StatusCode, String)> {
    // Validate input.
    if body.username.len() < 3 || body.username.len() > 64 {
        return Err((StatusCode::BAD_REQUEST, "Username must be 3-64 characters".into()));
    }
    if body.password.len() < 6 {
        return Err((StatusCode::BAD_REQUEST, "Password must be at least 6 characters".into()));
    }

    // Check for existing user.
    if repo::get_player_by_username(&state.db, &body.username).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .is_some()
    {
        return Err((StatusCode::CONFLICT, "Username already taken".into()));
    }

    // Hash password.
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(body.password.as_bytes(), &salt)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .to_string();

    // Create player.
    let player_id = Uuid::new_v4();
    repo::create_player(&state.db, player_id, &body.username, &hash)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Issue JWT.
    let token = issue_token(player_id, &body.username)?;

    Ok(Json(AuthResponse {
        token,
        player_id,
        username: body.username,
    }))
}

pub async fn login(
    State(state): State<SharedState>,
    Json(body): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, (StatusCode, String)> {
    let player = repo::get_player_by_username(&state.db, &body.username)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::UNAUTHORIZED, "Invalid credentials".into()))?;

    // Verify password.
    let parsed_hash = PasswordHash::new(&player.password_hash)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Argon2::default()
        .verify_password(body.password.as_bytes(), &parsed_hash)
        .map_err(|_| (StatusCode::UNAUTHORIZED, "Invalid credentials".to_string()))?;

    let token = issue_token(player.id, &player.username)?;

    Ok(Json(AuthResponse {
        token,
        player_id: player.id,
        username: player.username,
    }))
}

fn issue_token(player_id: Uuid, username: &str) -> Result<String, (StatusCode, String)> {
    let expiration = Utc::now()
        .checked_add_signed(chrono::Duration::hours(24))
        .unwrap()
        .timestamp() as usize;

    let claims = Claims {
        sub: player_id,
        username: username.to_string(),
        exp: expiration,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(&jwt_secret()),
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}
