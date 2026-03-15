use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// An executed trade between a buyer and seller.
/// Created when the matching engine fills (fully or partially) two orders.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub id: Uuid,
    pub buy_order_id: Uuid,
    pub sell_order_id: Uuid,
    pub resource_id: Uuid,
    pub buyer_id: Uuid,
    pub seller_id: Uuid,
    /// Price per unit at which the trade executed.
    pub price: Decimal,
    /// Number of units exchanged.
    pub quantity: Decimal,
    /// Total value = price * quantity.
    pub total_value: Decimal,
    pub executed_at: DateTime<Utc>,
}
