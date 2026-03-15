//! Shared application state.
//!
//! Held in an `Arc` and passed to all handlers.

use std::sync::Arc;
use sqlx::PgPool;
use bb8_redis::{bb8::Pool, RedisConnectionManager};
use tokio::sync::broadcast;
use uuid::Uuid;

const BROADCAST_CAPACITY: usize = 1024;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerEvent {
    TradeExecuted {
        resource_slug: String,
        price: String,
        quantity: String,
        buyer: String,
        seller: String,
    },
    MarketUpdate {
        markets: Vec<MarketBrief>,
    },
    ChatMessage {
        username: String,
        message: String,
        timestamp: String,
    },
    Notification {
        player_id: Uuid,
        message: String,
    },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MarketBrief {
    pub slug: String,
    pub price: String,
    pub ema: String,
    pub supply: String,
    pub demand: String,
}

pub struct AppState {
    pub db: PgPool,
    pub redis_pool: Pool<RedisConnectionManager>,
    /// Local channel to distribute Redis messages to connected WebSockets on this specific instance.
    pub local_ws_tx: broadcast::Sender<String>,
}

impl AppState {
    pub fn new(db: PgPool, redis_pool: Pool<RedisConnectionManager>) -> Self {
        let (local_ws_tx, _) = broadcast::channel(BROADCAST_CAPACITY);
        Self {
            db,
            redis_pool,
            local_ws_tx,
        }
    }

    /// Veröffentlicht ein Event im zentralen Redis-Cluster.
    pub async fn broadcast_to_redis(&self, event: ServerEvent) -> anyhow::Result<()> {
        let mut conn = self.redis_pool.get().await?;
        let payload = serde_json::to_string(&event)?;

        redis::cmd("PUBLISH")
            .arg("econwar:ws:broadcast")
            .arg(payload)
            .query_async(&mut *conn)
            .await?;

        Ok(())
    }

    pub fn subscribe_local(&self) -> broadcast::Receiver<String> {
        self.local_ws_tx.subscribe()
    }
}

pub type SharedState = Arc<AppState>;