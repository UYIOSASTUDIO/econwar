use rust_decimal::Decimal;
use uuid::Uuid;
use chrono::Utc;

use crate::models::{TradeOrder, OrderStatus, Transaction};

/// Order-book matching engine.
///
/// Uses a price-time priority algorithm:
///   1. Buy orders sorted by price DESC, then time ASC  (highest bid first)
///   2. Sell orders sorted by price ASC, then time ASC  (lowest ask first)
///   3. A match occurs when best_bid >= best_ask
///   4. Execution price = the older order's price (maker gets their price)
///
/// The engine is stateless — it takes two slices of orders and returns
/// the fills plus mutated order states.
pub struct MatchingEngine;

/// Result of running the matching engine on a resource's order book.
#[derive(Debug, Default)]
pub struct MatchResult {
    /// Newly created transactions (fills).
    pub transactions: Vec<Transaction>,
    /// Orders whose status or quantity changed.
    pub updated_orders: Vec<TradeOrder>,
    /// The last execution price (if any trades occurred).
    pub last_price: Option<Decimal>,
}

impl MatchingEngine {
    /// Match all open orders for a single resource.
    ///
    /// `buys` and `sells` must be pre-sorted (the caller handles this
    /// so the engine stays allocation-free where possible).
    pub fn match_orders(
        buys: &mut Vec<TradeOrder>,
        sells: &mut Vec<TradeOrder>,
    ) -> MatchResult {
        // Sort: buys by price DESC then created_at ASC.
        buys.sort_by(|a, b| {
            b.price.cmp(&a.price).then(a.created_at.cmp(&b.created_at))
        });
        // Sort: sells by price ASC then created_at ASC.
        sells.sort_by(|a, b| {
            a.price.cmp(&b.price).then(a.created_at.cmp(&b.created_at))
        });

        let mut result = MatchResult::default();
        let mut buy_idx = 0;
        let mut sell_idx = 0;

        while buy_idx < buys.len() && sell_idx < sells.len() {
            let buy = &mut buys[buy_idx];
            let sell = &mut sells[sell_idx];

            // No match if best bid < best ask.
            if buy.price < sell.price {
                break;
            }

            // Prevent self-trading.
            if buy.player_id == sell.player_id {
                sell_idx += 1;
                continue;
            }

            // Execution price: the older (maker) order's price.
            let exec_price = if buy.created_at <= sell.created_at {
                buy.price
            } else {
                sell.price
            };

            // Fill quantity is the minimum of both remaining quantities.
            let fill_qty = buy.quantity.min(sell.quantity);

            // Create the transaction record.
            let txn = Transaction {
                id: Uuid::new_v4(),
                buy_order_id: buy.id,
                sell_order_id: sell.id,
                resource_id: buy.resource_id,
                buyer_id: buy.player_id,
                seller_id: sell.player_id,
                price: exec_price,
                quantity: fill_qty,
                total_value: exec_price * fill_qty,
                executed_at: Utc::now(),
            };
            result.last_price = Some(exec_price);
            result.transactions.push(txn);

            // Decrement remaining quantities.
            buy.quantity -= fill_qty;
            sell.quantity -= fill_qty;

            // Update order statuses.
            if buy.quantity.is_zero() {
                buy.status = OrderStatus::Filled;
                buy_idx += 1;
            } else {
                buy.status = OrderStatus::PartiallyFilled;
            }

            if sell.quantity.is_zero() {
                sell.status = OrderStatus::Filled;
                sell_idx += 1;
            } else {
                sell.status = OrderStatus::PartiallyFilled;
            }
        }

        // Collect all mutated orders for persistence.
        for order in buys.iter().chain(sells.iter()) {
            if order.status != OrderStatus::Open {
                result.updated_orders.push(order.clone());
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn make_order(
        order_type: OrderType,
        price: Decimal,
        quantity: Decimal,
        player_id: Uuid,
    ) -> TradeOrder {
        TradeOrder {
            id: Uuid::new_v4(),
            player_id,
            company_id: Uuid::new_v4(),
            resource_id: Uuid::new_v4(),
            order_type,
            price,
            quantity,
            original_quantity: quantity,
            status: OrderStatus::Open,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn test_basic_match() {
        let buyer = Uuid::new_v4();
        let seller = Uuid::new_v4();
        let mut buys = vec![make_order(OrderType::Buy, dec!(100), dec!(10), buyer)];
        let mut sells = vec![make_order(OrderType::Sell, dec!(95), dec!(10), seller)];

        let result = MatchingEngine::match_orders(&mut buys, &mut sells);

        assert_eq!(result.transactions.len(), 1);
        let txn = &result.transactions[0];
        assert_eq!(txn.quantity, dec!(10));
        // Execution at maker (older) price.
        assert!(txn.price == dec!(100) || txn.price == dec!(95));
    }

    #[test]
    fn test_no_match_when_bid_below_ask() {
        let buyer = Uuid::new_v4();
        let seller = Uuid::new_v4();
        let mut buys = vec![make_order(OrderType::Buy, dec!(90), dec!(10), buyer)];
        let mut sells = vec![make_order(OrderType::Sell, dec!(95), dec!(10), seller)];

        let result = MatchingEngine::match_orders(&mut buys, &mut sells);
        assert!(result.transactions.is_empty());
    }

    #[test]
    fn test_partial_fill() {
        let buyer = Uuid::new_v4();
        let seller = Uuid::new_v4();
        let mut buys = vec![make_order(OrderType::Buy, dec!(100), dec!(20), buyer)];
        let mut sells = vec![make_order(OrderType::Sell, dec!(95), dec!(10), seller)];

        let result = MatchingEngine::match_orders(&mut buys, &mut sells);

        assert_eq!(result.transactions.len(), 1);
        assert_eq!(result.transactions[0].quantity, dec!(10));
        assert_eq!(buys[0].quantity, dec!(10)); // 10 remaining
        assert_eq!(buys[0].status, OrderStatus::PartiallyFilled);
        assert_eq!(sells[0].quantity, dec!(0));
        assert_eq!(sells[0].status, OrderStatus::Filled);
    }
}
