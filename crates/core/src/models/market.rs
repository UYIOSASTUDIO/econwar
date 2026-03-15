use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A single market exists for each resource.
/// Prices are derived from the order book, not set by the server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Market {
    pub id: Uuid,
    pub resource_id: Uuid,
    /// Last executed trade price — the "current price" displayed to players.
    pub last_price: Decimal,
    /// Exponential moving average price (smoothed signal for analytics).
    pub ema_price: Decimal,
    /// Total units available across all sell orders.
    pub total_supply: Decimal,
    /// Total units requested across all buy orders.
    pub total_demand: Decimal,
    /// Cumulative volume traded since market creation.
    pub total_volume: Decimal,
    pub updated_at: DateTime<Utc>,
}

/// An open order on the order book.
/// We use a simple limit-order model: orders sit on the book until
/// matched or cancelled.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeOrder {
    pub id: Uuid,
    pub player_id: Uuid,
    pub company_id: Uuid,
    pub resource_id: Uuid,
    pub order_type: OrderType,
    /// Price per unit the player is willing to pay / accept.
    pub price: Decimal,
    /// Remaining quantity (decremented as partial fills occur).
    pub quantity: Decimal,
    /// Original quantity for display purposes.
    pub original_quantity: Decimal,
    pub status: OrderStatus,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "varchar", rename_all = "snake_case")]
pub enum OrderType {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "varchar", rename_all = "snake_case")]
pub enum OrderStatus {
    Open,
    PartiallyFilled,
    Filled,
    Cancelled,
}

/// A snapshot of market data at a point in time, used for price history charts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketSnapshot {
    pub id: Uuid,
    pub resource_id: Uuid,
    pub price: Decimal,
    pub volume: Decimal,
    pub supply: Decimal,
    pub demand: Decimal,
    pub recorded_at: DateTime<Utc>,
}
