use chrono::Utc;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::models::*;
use super::matching::{MatchingEngine, MatchResult};
use super::pricing::PricingEngine;
use super::production::{ProductionEngine, TickResult};

/// Top-level simulation coordinator.
///
/// The EconomicEngine orchestrates a full simulation tick:
///   1. Advance production jobs → produce outputs
///   2. Pay wages → deduct from company treasuries
///   3. Spawn NPC orders for raw materials
///   4. Run the matching engine on every active market
///   5. Record market snapshots
///
/// The engine is designed to be called once per tick interval (e.g. every 5 seconds).
/// It operates on in-memory state snapshots provided by the database layer.
pub struct EconomicEngine {
    pub tick_count: u64,
}

/// The full set of side-effects produced by one simulation tick.
/// The server applies these transactionally to the database.
#[derive(Debug, Default, Clone)]
pub struct TickEffects {
    // ── Von der API injizierte Effekte ───────────────────────────────
    pub new_companies: Vec<Company>,
    pub updated_companies: Vec<Company>,
    pub new_jobs: Vec<ProductionJob>,
    pub new_orders: Vec<TradeOrder>,
    pub cancelled_orders: Vec<Uuid>,

    // ── Von der Engine berechnete Effekte ────────────────────────────
    pub updated_jobs: Vec<ProductionJob>,
    pub inventory_deltas: Vec<(Uuid, Uuid, Decimal)>,
    pub treasury_deltas: Vec<(Uuid, Decimal)>,
    pub npc_orders: Vec<TradeOrder>,
    pub match_results: Vec<(Uuid, MatchResult)>,
    pub snapshots: Vec<MarketSnapshot>,
    pub updated_markets: Vec<Market>,
}

impl EconomicEngine {
    pub fn new() -> Self {
        Self { tick_count: 0 }
    }

    /// Execute one full simulation tick.
    pub fn tick(
        &mut self,
        companies: &[Company],
        jobs: &mut [ProductionJob],
        recipes: &[Recipe],
        resources: &[Resource],
        markets: &mut [Market],
        buy_orders: &mut Vec<TradeOrder>,
        sell_orders: &mut Vec<TradeOrder>,
    ) -> TickEffects {
        self.tick_count += 1;
        let mut effects = TickEffects::default();

        // ── Phase 1: Advance production ─────────────────────────────────
        for job in jobs.iter_mut() {
            if job.status != ProductionStatus::Running {
                continue;
            }
            let recipe = match recipes.iter().find(|r| r.id == job.recipe_id) {
                Some(r) => r,
                None => continue,
            };
            match ProductionEngine::tick_job(job, recipe) {
                TickResult::Completed { outputs, workers_freed: _ } => {
                    for (resource_id, qty) in outputs {
                        effects.inventory_deltas.push((job.company_id, resource_id, qty));
                    }
                    effects.updated_jobs.push(job.clone());
                }
                TickResult::InProgress(_) => {
                    effects.updated_jobs.push(job.clone());
                }
                TickResult::NoOp => {}
            }
        }

        // ── Phase 2: Pay wages ──────────────────────────────────────────
        for company in companies {
            let wage = company.daily_wage_cost();
            if !wage.is_zero() {
                effects.treasury_deltas.push((company.id, -wage));
            }
        }

        // ── Phase 3: NPC resource spawning ──────────────────────────────
        // Raw materials slowly appear as NPC sell orders, simulating
        // extraction industries.  This ensures markets never completely
        // dry up, while keeping quantities small so player production
        // is still the primary supply source.
        for resource in resources {
            if resource.category != ResourceCategory::RawMaterial {
                continue;
            }
            if resource.spawn_rate.is_zero() {
                continue;
            }

            let floor_price = PricingEngine::npc_floor_price(resource.base_price);
            let npc_order = TradeOrder {
                id: Uuid::new_v4(),
                // NPC uses nil UUID as the player/company ID.
                player_id: Uuid::nil(),
                company_id: Uuid::nil(),
                resource_id: resource.id,
                order_type: OrderType::Sell,
                price: floor_price,
                quantity: resource.spawn_rate,
                original_quantity: resource.spawn_rate,
                status: OrderStatus::Open,
                created_at: Utc::now(),
            };
            effects.npc_orders.push(npc_order.clone());
            sell_orders.push(npc_order);
        }

        // ── Phase 4: Run matching engine per resource ───────────────────
        for market in markets.iter_mut() {
            let rid = market.resource_id;

            let mut res_buys: Vec<TradeOrder> = buy_orders
                .iter()
                .filter(|o| o.resource_id == rid && o.status == OrderStatus::Open)
                .cloned()
                .collect();
            let mut res_sells: Vec<TradeOrder> = sell_orders
                .iter()
                .filter(|o| o.resource_id == rid && o.status == OrderStatus::Open)
                .cloned()
                .collect();

            let result = MatchingEngine::match_orders(&mut res_buys, &mut res_sells);

            // Update market price if trades occurred.
            if let Some(last_price) = result.last_price {
                PricingEngine::update_ema(market, last_price);
                let volume: Decimal = result.transactions.iter().map(|t| t.quantity).sum();
                market.total_volume += volume;
            }

            // Recalculate supply/demand from remaining open quantities.
            let remaining_supply: Decimal = res_sells
                .iter()
                .filter(|o| o.status == OrderStatus::Open || o.status == OrderStatus::PartiallyFilled)
                .map(|o| o.quantity)
                .sum();
            let remaining_demand: Decimal = res_buys
                .iter()
                .filter(|o| o.status == OrderStatus::Open || o.status == OrderStatus::PartiallyFilled)
                .map(|o| o.quantity)
                .sum();
            PricingEngine::recalculate_supply_demand(market, remaining_supply, remaining_demand);
            market.updated_at = Utc::now();

            effects.updated_markets.push(market.clone());

            // Transfer funds and resources for each executed transaction.
            for txn in &result.transactions {
                // Buyer pays: deduct from buyer company treasury.
                effects.treasury_deltas.push((txn.buyer_id, -txn.total_value));
                // Seller receives: add to seller company treasury.
                effects.treasury_deltas.push((txn.seller_id, txn.total_value));
                // Buyer receives resource.
                effects.inventory_deltas.push((txn.buyer_id, txn.resource_id, txn.quantity));
                // Seller loses resource (already deducted when order was placed).
            }

            effects.match_results.push((rid, result));

            // ── Phase 5: Record snapshot ────────────────────────────────
            effects.snapshots.push(MarketSnapshot {
                id: Uuid::new_v4(),
                resource_id: rid,
                price: market.last_price,
                volume: market.total_volume,
                supply: market.total_supply,
                demand: market.total_demand,
                recorded_at: Utc::now(),
            });
        }

        effects
    }
}
