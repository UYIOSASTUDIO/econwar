//! Background economic simulation loop.
//!
//! Runs on a dedicated Tokio task.  Every `tick_interval_ms` milliseconds:
//!   1. Loads relevant state from the database
//!   2. Runs the economic engine tick
//!   3. Persists all side-effects back to the database
//!   4. Broadcasts market updates to WebSocket clients

use std::sync::Arc;
use std::time::Duration;

use econwar_core::engine::EconomicEngine;
use econwar_db::repo;

use crate::state::{AppState, MarketBrief, ServerEvent};

pub async fn run(state: Arc<AppState>, tick_interval_ms: u64) {
    let mut engine = EconomicEngine::new();
    let interval = Duration::from_millis(tick_interval_ms);

    tracing::info!("Game loop started (tick every {}ms)", tick_interval_ms);

    loop {
        tokio::time::sleep(interval).await;

        if let Err(e) = tick_once(&mut engine, &state).await {
            tracing::error!("Tick error: {e:#}");
        }
    }
}

async fn tick_once(engine: &mut EconomicEngine, state: &Arc<AppState>) -> anyhow::Result<()> {
    let pool = &state.db;

    // ── Load state ──────────────────────────────────────────────────
    let companies = repo::get_all_companies_for_tick(pool).await;
    let recipes = repo::get_all_recipes(pool).await?;
    let resources = repo::get_all_resources(pool).await?;
    let mut markets = repo::get_all_markets(pool).await?;
    let mut jobs = repo::get_running_jobs(pool).await?;

    // Load all open orders.
    let mut all_buys = Vec::new();
    let mut all_sells = Vec::new();
    for market in &markets {
        let mut buys = repo::get_open_orders_by_resource(pool, market.resource_id, "buy").await?;
        let mut sells = repo::get_open_orders_by_resource(pool, market.resource_id, "sell").await?;
        all_buys.append(&mut buys);
        all_sells.append(&mut sells);
    }

    // ── Run simulation tick ─────────────────────────────────────────
    let companies_vec = companies.unwrap_or_default();
    let effects = engine.tick(
        &companies_vec,
        &mut jobs,
        &recipes,
        &resources,
        &mut markets,
        &mut all_buys,
        &mut all_sells,
    );

    // ── Persist effects ─────────────────────────────────────────────

    // Updated production jobs.
    for job in &effects.updated_jobs {
        let _ = repo::update_production_job(pool, job).await;
    }

    // Inventory deltas.
    for (owner_id, resource_id, delta) in &effects.inventory_deltas {
        let _ = repo::upsert_inventory(pool, *owner_id, *resource_id, *delta).await;
    }

    // Treasury deltas.
    for (company_id, delta) in &effects.treasury_deltas {
        let _ = repo::update_company_treasury(pool, *company_id, *delta).await;
    }

    // NPC orders.
    for order in &effects.npc_orders {
        let _ = repo::insert_trade_order(pool, order).await;
    }

    // Transactions from matching.
    for (_rid, match_result) in &effects.match_results {
        for txn in &match_result.transactions {
            let _ = repo::insert_transaction(pool, txn).await;
        }
        for order in &match_result.updated_orders {
            let status_str = format!("{:?}", order.status).to_lowercase();
            let _ = repo::update_order_status(pool, order.id, order.quantity, &status_str).await;
        }
    }

    // Market updates.
    for market in &effects.updated_markets {
        let _ = repo::update_market(pool, market).await;
    }

    // Snapshots.
    for snap in &effects.snapshots {
        let _ = repo::insert_snapshot(pool, snap).await;
    }

    // ── Broadcast market update to WS clients ───────────────────────
    let slugs: std::collections::HashMap<uuid::Uuid, String> = resources
        .iter()
        .map(|r| (r.id, r.slug.clone()))
        .collect();

    let briefs: Vec<MarketBrief> = effects
        .updated_markets
        .iter()
        .filter_map(|m| {
            slugs.get(&m.resource_id).map(|slug| MarketBrief {
                slug: slug.clone(),
                price: m.last_price.to_string(),
                ema: m.ema_price.to_string(),
                supply: m.total_supply.to_string(),
                demand: m.total_demand.to_string(),
            })
        })
        .collect();

    if !briefs.is_empty() {
        state.broadcast(ServerEvent::MarketUpdate { markets: briefs });
    }

    tracing::debug!("Tick #{} complete", engine.tick_count);
    Ok(())
}
