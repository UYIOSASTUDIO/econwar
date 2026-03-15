//! Asynchronous write-behind persistence layer.
//!
//! Receives tick batches from the GameLoopActor and writes them to PostgreSQL
//! strictly using transactional boundaries to guarantee ACID compliance.

use std::sync::Arc;
use tokio::sync::mpsc;
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

    /// Konsumiert Batches kontinuierlich aus dem MPSC-Channel.
    pub async fn run(mut self) {
        tracing::info!("DbWriterActor started. Ready to persist tick batches.");

        while let Some(batch) = self.batch_rx.recv().await {
            let tick_count = batch.tick_count;
            if let Err(err) = self.persist_batch(batch).await {
                // Hier muss in Zukunft ein Circuit Breaker oder DLQ (Dead Letter Queue) greifen.
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

        // 1. Updated production jobs
        for job in &batch.effects.updated_jobs {
            sqlx::query("UPDATE production_jobs SET ticks_remaining = $1, status = $2 WHERE id = $3")
                .bind(job.ticks_remaining)
                .bind(format!("{:?}", job.status).to_lowercase())
                .bind(job.id)
                .execute(&mut *tx)
                .await?;
        }

        // 2. Inventory deltas (Upsert)
        for (owner_id, resource_id, delta) in &batch.effects.inventory_deltas {
            sqlx::query(
                r#"INSERT INTO inventories (id, owner_id, resource_id, quantity)
                   VALUES ($1, $2, $3, $4)
                   ON CONFLICT (owner_id, resource_id)
                   DO UPDATE SET quantity = inventories.quantity + $4"#
            )
                .bind(uuid::Uuid::new_v4())
                .bind(owner_id)
                .bind(resource_id)
                .bind(delta)
                .execute(&mut *tx)
                .await?;
        }

        // 3. Treasury deltas
        for (company_id, delta) in &batch.effects.treasury_deltas {
            sqlx::query("UPDATE companies SET treasury = treasury + $1 WHERE id = $2")
                .bind(delta)
                .bind(company_id)
                .execute(&mut *tx)
                .await?;
        }

        // 4. NPC Orders
        for order in &batch.effects.npc_orders {
            sqlx::query(
                r#"INSERT INTO trade_orders
                   (id, player_id, company_id, resource_id, order_type, price, quantity, original_quantity, status, created_at)
                   VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)"#
            )
                .bind(order.id)
                .bind(order.player_id)
                .bind(order.company_id)
                .bind(order.resource_id)
                .bind(format!("{:?}", order.order_type).to_lowercase())
                .bind(order.price)
                .bind(order.quantity)
                .bind(order.original_quantity)
                .bind(format!("{:?}", order.status).to_lowercase())
                .bind(order.created_at)
                .execute(&mut *tx)
                .await?;
        }

        // 5. Transactions & Order Updates
        for (_rid, match_result) in &batch.effects.match_results {
            for transaction in &match_result.transactions {
                sqlx::query(
                    r#"INSERT INTO transactions
                       (id, buy_order_id, sell_order_id, resource_id, buyer_id, seller_id, price, quantity, total_value, executed_at)
                       VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)"#
                )
                    .bind(transaction.id)
                    .bind(transaction.buy_order_id)
                    .bind(transaction.sell_order_id)
                    .bind(transaction.resource_id)
                    .bind(transaction.buyer_id)
                    .bind(transaction.seller_id)
                    .bind(transaction.price)
                    .bind(transaction.quantity)
                    .bind(transaction.total_value)
                    .bind(transaction.executed_at)
                    .execute(&mut *tx)
                    .await?;
            }

            for order in &match_result.updated_orders {
                sqlx::query("UPDATE trade_orders SET quantity = $1, status = $2 WHERE id = $3")
                    .bind(order.quantity)
                    .bind(format!("{:?}", order.status).to_lowercase())
                    .bind(order.id)
                    .execute(&mut *tx)
                    .await?;
            }
        }

        // 6. Updated Markets
        for market in &batch.effects.updated_markets {
            sqlx::query(
                r#"UPDATE markets SET last_price = $1, ema_price = $2,
                   total_supply = $3, total_demand = $4, total_volume = $5, updated_at = $6
                   WHERE id = $7"#
            )
                .bind(market.last_price)
                .bind(market.ema_price)
                .bind(market.total_supply)
                .bind(market.total_demand)
                .bind(market.total_volume)
                .bind(market.updated_at)
                .bind(market.id)
                .execute(&mut *tx)
                .await?;
        }

        // 7. Market Snapshots
        for snap in &batch.effects.snapshots {
            sqlx::query(
                r#"INSERT INTO market_snapshots (id, resource_id, price, volume, supply, demand, recorded_at)
                   VALUES ($1, $2, $3, $4, $5, $6, $7)"#
            )
                .bind(snap.id)
                .bind(snap.resource_id)
                .bind(snap.price)
                .bind(snap.volume)
                .bind(snap.supply)
                .bind(snap.demand)
                .bind(snap.recorded_at)
                .execute(&mut *tx)
                .await?;
        }

        tx.commit().await?;

        Ok(())
    }
}