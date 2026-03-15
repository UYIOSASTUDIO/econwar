use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A player-owned company that produces and trades goods.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Company {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub name: String,
    /// Operating capital held by the company.
    pub treasury: Decimal,
    /// Number of workers currently employed.
    pub workers: i32,
    /// Maximum workers the company can hire (scales with factories).
    pub worker_capacity: i32,
    /// Number of factories (each factory adds production capacity).
    pub factories: i32,
    /// Technology level — unlocks advanced recipes and efficiency bonuses.
    pub tech_level: i32,
    pub created_at: DateTime<Utc>,
}

/// The cost to build one additional factory.
/// Scales quadratically so expansion gets progressively harder.
impl Company {
    pub fn next_factory_cost(&self) -> Decimal {
        let base = Decimal::from(10_000);
        let multiplier = Decimal::from((self.factories + 1).pow(2));
        base * multiplier
    }

    /// Workers a single factory can support.
    pub const WORKERS_PER_FACTORY: i32 = 50;

    /// Recalculate worker capacity from factory count.
    pub fn recalculate_capacity(&mut self) {
        self.worker_capacity = self.factories * Self::WORKERS_PER_FACTORY;
    }

    /// Daily wage cost for all employed workers.
    pub fn daily_wage_cost(&self) -> Decimal {
        // Base wage: 100 credits per worker per tick.
        Decimal::from(100) * Decimal::from(self.workers)
    }
}
