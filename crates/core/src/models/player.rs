use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use rust_decimal::Decimal;

/// A player account in the game world.
/// Each player can own multiple companies.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Player {
    pub id: Uuid,
    pub username: String,
    /// Hashed password — never serialized to clients.
    #[serde(skip_serializing)]
    pub password_hash: String,
    /// Cash balance available for direct spending (not tied to a company).
    pub balance: Decimal,
    pub created_at: DateTime<Utc>,
    pub last_login: DateTime<Utc>,
    pub is_online: bool,
}

/// Lightweight view of a player sent over the wire.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerPublic {
    pub id: Uuid,
    pub username: String,
    pub balance: Decimal,
    pub is_online: bool,
}

impl From<&Player> for PlayerPublic {
    fn from(p: &Player) -> Self {
        Self {
            id: p.id,
            username: p.username.clone(),
            balance: p.balance,
            is_online: p.is_online,
        }
    }
}
