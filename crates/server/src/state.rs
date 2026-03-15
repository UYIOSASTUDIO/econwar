//! Shared application state.
//!
//! Held in an `Arc` and passed to all handlers.

use std::sync::Arc;

use dashmap::DashMap;
use sqlx::PgPool;
use tokio::sync::broadcast;
use uuid::Uuid;

/// Broadcast channel capacity for real-time events.
const BROADCAST_CAPACITY: usize = 1024;

/// Events pushed to all connected WebSocket clients.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerEvent {
    /// A trade was executed on a market.
    TradeExecuted {
        resource_slug: String,
        price: String,
        quantity: String,
        buyer: String,
        seller: String,
    },
    /// Market prices updated (sent every tick).
    MarketUpdate {
        markets: Vec<MarketBrief>,
    },
    /// Global chat message.
    ChatMessage {
        username: String,
        message: String,
        timestamp: String,
    },
    /// A player-specific notification.
    Notification {
        player_id: Uuid,
        message: String,
    },
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct MarketBrief {
    pub slug: String,
    pub price: String,
    pub ema: String,
    pub supply: String,
    pub demand: String,
}

pub struct AppState {
    pub db: PgPool,
    /// Broadcast channel for real-time events to all WS clients.
    pub events_tx: broadcast::Sender<ServerEvent>,
    /// Online player sessions: player_id → username.
    pub sessions: DashMap<Uuid, String>,
}

impl AppState {
    pub fn new(db: PgPool) -> Self {
        let (events_tx, _) = broadcast::channel(BROADCAST_CAPACITY);
        Self {
            db,
            events_tx,
            sessions: DashMap::new(),
        }
    }

    /// Subscribe to the real-time event stream.
    pub fn subscribe(&self) -> broadcast::Receiver<ServerEvent> {
        self.events_tx.subscribe()
    }

    /// Broadcast an event to all connected clients.
    pub fn broadcast(&self, event: ServerEvent) {
        // Ignore send errors (no subscribers).
        let _ = self.events_tx.send(event);
    }
}

/// Type alias used in Axum extractors.
pub type SharedState = Arc<AppState>;
