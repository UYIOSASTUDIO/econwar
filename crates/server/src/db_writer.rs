//! Asynchronous write-behind persistence layer.
//!
//! Uses highly optimized bulk operations (QueryBuilder & UNNEST)
//! to eliminate the N+1 query problem and maximize database throughput.
//! Includes automated deduplication and chunking to respect PostgreSQL limits.

use std::sync::Arc;
use tokio::sync::mpsc;
use sqlx::{Postgres, QueryBuilder};
use crate::state::AppState;
use crate::game_loop::TickBatch;

pub struct DbWriterActor {
    state: Arc<AppState>,
    batch_rx: mpsc::Receiver<TickBatch>,
}

impl DbWriterActor {
    pub fn new(state: Arc<AppState>, batch_rx: mpsc::Receiver<TickBatch>) -> Self {
        Self { state, batch_rx }
    }

    pub async fn run(mut self) {
        tracing::info!("DbWriterActor started. Ready to persist tick batches.");

        while let Some(batch) = self.batch_rx.recv().await {
            let tick_count = batch.tick_count;
            if let Err(err) = self.persist_batch(batch).await {
                tracing::error!("CRITICAL: Transaction failed for tick {}: {}", tick_count, err);
            } else {
                tracing::debug!("Successfully persisted tick {}", tick_count);
            }
        }

        tracing::warn!("DbWriterActor shutting down (Channel closed).");
    }

    async fn persist_batch(&self, batch: TickBatch) -> anyhow::Result<()> {
        let pool = &self.state.db;
        let mut tx = pool.begin().await?;
        let effects = &batch.effects;

        // ─── 1. Aggregation & Deduplication ──────────────────────────────

        let mut agg_inv = std::collections::HashMap::new();
        for (owner_id, resource_id, delta) in &effects.inventory_deltas {
            *agg_inv.entry((*owner_id, *resource_id)).or_insert(rust_decimal::Decimal::ZERO) += delta;
        }

        let mut agg_treasury = std::collections::HashMap::new();
        for (company_id, delta) in &effects.treasury_deltas {
            *agg_treasury.entry(*company_id).or_insert(rust_decimal::Decimal::ZERO) += delta;
        }

        let mut unique_companies = std::collections::HashMap::new();
        for c in &effects.updated_companies {
            unique_companies.insert(c.id, c);
        }

        let mut all_transactions = Vec::new();
        let mut all_updated_orders = Vec::new();
        for (_, result) in &effects.match_results {
            all_transactions.extend(&result.transactions);
            all_updated_orders.extend(&result.updated_orders);
        }

        let all_new_orders: Vec<_> = effects.new_orders.iter().chain(effects.npc_orders.iter()).collect();

        // ─── 2. Bulk Execution with Chunking ─────────────────────────────
        // PostgreSQL max parameters: 65535. Chunk sizes = 65000 / columns

        // 2.1 New Companies (9 columns -> 7000 chunks)
        if !effects.new_companies.is_empty() {
            for chunk in effects.new_companies.chunks(7000) {
                let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(
                    "INSERT INTO companies (id, owner_id, name, treasury, workers, worker_capacity, factories, tech_level, created_at) "
                );
                qb.push_values(chunk, |mut b, c| {
                    b.push_bind(c.id).push_bind(c.owner_id).push_bind(c.name.clone())
                        .push_bind(c.treasury).push_bind(c.workers).push_bind(c.worker_capacity)
                        .push_bind(c.factories).push_bind(c.tech_level).push_bind(c.created_at);
                });
                qb.build().execute(&mut *tx).await?;
            }
        }

        // 2.2 Updated Companies (UPSERT)
        let updated_companies_vals: Vec<_> = unique_companies.values().collect();
        if !updated_companies_vals.is_empty() {
            for chunk in updated_companies_vals.chunks(7000) {
                let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(
                    "INSERT INTO companies (id, owner_id, name, treasury, workers, worker_capacity, factories, tech_level, created_at) "
                );
                qb.push_values(chunk, |mut b, c| {
                    b.push_bind(c.id).push_bind(c.owner_id).push_bind(c.name.clone())
                        .push_bind(c.treasury).push_bind(c.workers).push_bind(c.worker_capacity)
                        .push_bind(c.factories).push_bind(c.tech_level).push_bind(c.created_at);
                });
                qb.push(
                    " ON CONFLICT (id) DO UPDATE SET
                      treasury = EXCLUDED.treasury,
                      workers = EXCLUDED.workers,
                      worker_capacity = EXCLUDED.worker_capacity,
                      factories = EXCLUDED.factories,
                      tech_level = EXCLUDED.tech_level"
                );
                qb.build().execute(&mut *tx).await?;
            }
        }

        // 2.3 New Jobs (6 columns -> 10000 chunks)
        if !effects.new_jobs.is_empty() {
            for chunk in effects.new_jobs.chunks(10000) {
                let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(
                    "INSERT INTO production_jobs (id, company_id, recipe_id, batch_size, ticks_remaining, status) "
                );
                qb.push_values(chunk, |mut b, j| {
                    b.push_bind(j.id).push_bind(j.company_id).push_bind(j.recipe_id)
                        .push_bind(j.batch_size).push_bind(j.ticks_remaining)
                        .push_bind(format!("{:?}", j.status).to_lowercase());
                });
                qb.build().execute(&mut *tx).await?;
            }
        }

        // 2.4 Updated Jobs (UPSERT)
        if !effects.updated_jobs.is_empty() {
            for chunk in effects.updated_jobs.chunks(10000) {
                let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(
                    "INSERT INTO production_jobs (id, company_id, recipe_id, batch_size, ticks_remaining, status) "
                );
                qb.push_values(chunk, |mut b, j| {
                    b.push_bind(j.id).push_bind(j.company_id).push_bind(j.recipe_id)
                        .push_bind(j.batch_size).push_bind(j.ticks_remaining)
                        .push_bind(format!("{:?}", j.status).to_lowercase());
                });
                qb.push(" ON CONFLICT (id) DO UPDATE SET ticks_remaining = EXCLUDED.ticks_remaining, status = EXCLUDED.status");
                qb.build().execute(&mut *tx).await?;
            }
        }

        // 2.5 New Orders & NPC Orders (10 columns -> 6000 chunks)
        if !all_new_orders.is_empty() {
            for chunk in all_new_orders.chunks(6000) {
                let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(
                    "INSERT INTO trade_orders (id, player_id, company_id, resource_id, order_type, price, quantity, original_quantity, status, created_at) "
                );
                qb.push_values(chunk, |mut b, o| {
                    b.push_bind(o.id).push_bind(o.player_id).push_bind(o.company_id).push_bind(o.resource_id)
                        .push_bind(format!("{:?}", o.order_type).to_lowercase()).push_bind(o.price)
                        .push_bind(o.quantity).push_bind(o.original_quantity)
                        .push_bind(format!("{:?}", o.status).to_lowercase()).push_bind(o.created_at);
                });
                qb.build().execute(&mut *tx).await?;
            }
        }

        // 2.6 Updated Orders (UPSERT)
        if !all_updated_orders.is_empty() {
            for chunk in all_updated_orders.chunks(6000) {
                let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(
                    "INSERT INTO trade_orders (id, player_id, company_id, resource_id, order_type, price, quantity, original_quantity, status, created_at) "
                );
                qb.push_values(chunk, |mut b, o| {
                    b.push_bind(o.id).push_bind(o.player_id).push_bind(o.company_id).push_bind(o.resource_id)
                        .push_bind(format!("{:?}", o.order_type).to_lowercase()).push_bind(o.price)
                        .push_bind(o.quantity).push_bind(o.original_quantity)
                        .push_bind(format!("{:?}", o.status).to_lowercase()).push_bind(o.created_at);
                });
                qb.push(" ON CONFLICT (id) DO UPDATE SET quantity = EXCLUDED.quantity, status = EXCLUDED.status");
                qb.build().execute(&mut *tx).await?;
            }
        }

        // 2.7 Cancelled Orders (PostgreSQL ANY operator for arrays)
        if !effects.cancelled_orders.is_empty() {
            sqlx::query("UPDATE trade_orders SET status = 'cancelled' WHERE id = ANY($1)")
                .bind(&effects.cancelled_orders)
                .execute(&mut *tx)
                .await?;
        }

        // 2.8 Inventory Deltas (4 columns -> 16000 chunks)
        if !agg_inv.is_empty() {
            let inv_entries: Vec<_> = agg_inv.into_iter().collect();
            for chunk in inv_entries.chunks(16000) {
                let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(
                    "INSERT INTO inventories (id, owner_id, resource_id, quantity) "
                );
                qb.push_values(chunk, |mut b, ((owner, res), qty)| {
                    b.push_bind(uuid::Uuid::new_v4()).push_bind(*owner).push_bind(*res).push_bind(*qty);
                });
                qb.push(" ON CONFLICT (owner_id, resource_id) DO UPDATE SET quantity = inventories.quantity + EXCLUDED.quantity");
                qb.build().execute(&mut *tx).await?;
            }
        }

        // 2.9 Treasury Deltas (High-Performance UNNEST Arrays)
        if !agg_treasury.is_empty() {
            let (ids, deltas): (Vec<_>, Vec<_>) = agg_treasury.into_iter().unzip();
            sqlx::query(
                "UPDATE companies SET treasury = treasury + data.delta
                 FROM (SELECT UNNEST($1::uuid[]) as id, UNNEST($2::numeric[]) as delta) AS data
                 WHERE companies.id = data.id"
            )
                .bind(&ids)
                .bind(&deltas)
                .execute(&mut *tx)
                .await?;
        }

        // 2.10 Transactions (10 columns -> 6000 chunks)
        if !all_transactions.is_empty() {
            for chunk in all_transactions.chunks(6000) {
                let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(
                    "INSERT INTO transactions (id, buy_order_id, sell_order_id, resource_id, buyer_id, seller_id, price, quantity, total_value, executed_at) "
                );
                qb.push_values(chunk, |mut b, t| {
                    b.push_bind(t.id).push_bind(t.buy_order_id).push_bind(t.sell_order_id)
                        .push_bind(t.resource_id).push_bind(t.buyer_id).push_bind(t.seller_id)
                        .push_bind(t.price).push_bind(t.quantity).push_bind(t.total_value).push_bind(t.executed_at);
                });
                qb.build().execute(&mut *tx).await?;
            }
        }

        // 2.11 Updated Markets (8 columns -> 8000 chunks)
        if !effects.updated_markets.is_empty() {
            for chunk in effects.updated_markets.chunks(8000) {
                let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(
                    "INSERT INTO markets (id, resource_id, last_price, ema_price, total_volume, total_supply, total_demand, updated_at) "
                );
                qb.push_values(chunk, |mut b, m| {
                    b.push_bind(m.id).push_bind(m.resource_id).push_bind(m.last_price).push_bind(m.ema_price)
                        .push_bind(m.total_volume).push_bind(m.total_supply).push_bind(m.total_demand).push_bind(m.updated_at);
                });
                qb.push(
                    " ON CONFLICT (id) DO UPDATE SET
                      last_price = EXCLUDED.last_price,
                      ema_price = EXCLUDED.ema_price,
                      total_volume = EXCLUDED.total_volume,
                      total_supply = EXCLUDED.total_supply,
                      total_demand = EXCLUDED.total_demand,
                      updated_at = EXCLUDED.updated_at"
                );
                qb.build().execute(&mut *tx).await?;
            }
        }

        // 2.12 Snapshots (7 columns -> 9000 chunks)
        if !effects.snapshots.is_empty() {
            for chunk in effects.snapshots.chunks(9000) {
                let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(
                    "INSERT INTO market_snapshots (id, resource_id, price, volume, supply, demand, recorded_at) "
                );
                qb.push_values(chunk, |mut b, s| {
                    b.push_bind(s.id).push_bind(s.resource_id).push_bind(s.price)
                        .push_bind(s.volume).push_bind(s.supply).push_bind(s.demand).push_bind(s.recorded_at);
                });
                qb.build().execute(&mut *tx).await?;
            }
        }

        tx.commit().await?;
        Ok(())
    }
}