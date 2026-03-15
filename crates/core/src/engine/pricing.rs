use rust_decimal::Decimal;
use rust_decimal_macros::dec;

use crate::models::Market;

/// Pricing engine computes derived market metrics.
///
/// Prices in EconWar are NOT set by the server.  They emerge from
/// player-placed orders on the order book.  The "price" shown on
/// the dashboard is simply the last execution price.
///
/// This module provides utilities for:
///   - EMA (exponential moving average) smoothing
///   - Supply/demand index calculation
///   - Volatility measurement
pub struct PricingEngine;

impl PricingEngine {
    /// EMA smoothing factor.  Lower = smoother, slower to react.
    /// 0.1 means the new price contributes 10% and history 90%.
    const EMA_ALPHA: Decimal = dec!(0.1);

    /// Update the market's EMA price after a new trade.
    pub fn update_ema(market: &mut Market, new_price: Decimal) {
        // EMA = alpha * new_price + (1 - alpha) * old_ema
        market.ema_price =
            Self::EMA_ALPHA * new_price + (dec!(1) - Self::EMA_ALPHA) * market.ema_price;
        market.last_price = new_price;
    }

    /// Supply/Demand ratio.
    /// > 1.0 means oversupply (bearish), < 1.0 means undersupply (bullish).
    /// Returns None if demand is zero to avoid division by zero.
    pub fn supply_demand_ratio(market: &Market) -> Option<Decimal> {
        if market.total_demand.is_zero() {
            return None;
        }
        Some(market.total_supply / market.total_demand)
    }

    /// Simple spread: best_ask - best_bid.
    /// A tight spread indicates a liquid, healthy market.
    pub fn spread(best_bid: Option<Decimal>, best_ask: Option<Decimal>) -> Option<Decimal> {
        match (best_bid, best_ask) {
            (Some(bid), Some(ask)) => Some(ask - bid),
            _ => None,
        }
    }

    /// Recalculate aggregate supply and demand from open orders.
    pub fn recalculate_supply_demand(
        market: &mut Market,
        total_sell_qty: Decimal,
        total_buy_qty: Decimal,
    ) {
        market.total_supply = total_sell_qty;
        market.total_demand = total_buy_qty;
    }

    /// NPC price floor: raw materials have a natural "extraction cost"
    /// below which NPC sellers won't go.  This prevents prices from
    /// collapsing to zero.
    pub fn npc_floor_price(base_price: Decimal) -> Decimal {
        base_price * dec!(0.5)
    }

    /// NPC price ceiling: if price exceeds 5x base, NPC sellers flood
    /// the market to dampen runaway inflation.
    pub fn npc_ceiling_price(base_price: Decimal) -> Decimal {
        base_price * dec!(5.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    fn test_market() -> Market {
        Market {
            id: Uuid::new_v4(),
            resource_id: Uuid::new_v4(),
            last_price: dec!(100),
            ema_price: dec!(100),
            total_supply: dec!(500),
            total_demand: dec!(400),
            total_volume: dec!(10000),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn test_ema_moves_toward_new_price() {
        let mut m = test_market();
        PricingEngine::update_ema(&mut m, dec!(120));
        // EMA should move toward 120 but not reach it.
        assert!(m.ema_price > dec!(100));
        assert!(m.ema_price < dec!(120));
    }

    #[test]
    fn test_supply_demand_ratio() {
        let m = test_market();
        let ratio = PricingEngine::supply_demand_ratio(&m).unwrap();
        assert_eq!(ratio, dec!(1.25)); // 500/400
    }
}
