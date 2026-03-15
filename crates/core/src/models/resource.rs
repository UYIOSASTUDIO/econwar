use serde::{Deserialize, Serialize};
use uuid::Uuid;
use rust_decimal::Decimal;

/// Every physical good in the game is a Resource.
/// Raw materials and finished products share the same type —
/// the distinction is whether the resource is an *input* or *output*
/// of a production recipe.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    pub id: Uuid,
    /// Machine-readable identifier, e.g. "copper", "electronics", "battery_pack".
    pub slug: String,
    /// Human-readable display name.
    pub name: String,
    /// Category for UI grouping.
    pub category: ResourceCategory,
    /// Base price used to seed the market on first launch.
    pub base_price: Decimal,
    /// Per-tick natural spawn rate (for raw materials only, 0 for manufactured goods).
    pub spawn_rate: Decimal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "varchar", rename_all = "snake_case")]
pub enum ResourceCategory {
    RawMaterial,
    Component,
    FinishedGood,
    Luxury,
}

/// How much of a resource a company or player currently holds.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Inventory {
    pub id: Uuid,
    pub owner_id: Uuid,        // company or player id
    pub resource_id: Uuid,
    pub quantity: Decimal,
}
