//! Repository functions — thin async wrappers around SQL queries.
//!
//! Organized by domain entity.  Each function takes a `&PgPool` and
//! returns domain types from `econwar_core::models`.

use econwar_core::models::*;
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

// ═══════════════════════════════════════════════════════════════════
//  PLAYERS
// ═══════════════════════════════════════════════════════════════════

pub async fn create_player(
    pool: &PgPool,
    id: Uuid,
    username: &str,
    password_hash: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO players (id, username, password_hash) VALUES ($1, $2, $3)"
    )
    .bind(id)
    .bind(username)
    .bind(password_hash)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_player_by_username(
    pool: &PgPool,
    username: &str,
) -> Result<Option<Player>, sqlx::Error> {
    let row = sqlx::query_as!(
        Player,
        r#"SELECT id, username, password_hash, balance, is_online,
                  created_at, last_login
           FROM players WHERE username = $1"#,
        username
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn get_player_by_id(
    pool: &PgPool,
    id: Uuid,
) -> Result<Option<Player>, sqlx::Error> {
    let row = sqlx::query_as!(
        Player,
        r#"SELECT id, username, password_hash, balance, is_online,
                  created_at, last_login
           FROM players WHERE id = $1"#,
        id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn update_player_balance(
    pool: &PgPool,
    player_id: Uuid,
    delta: Decimal,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE players SET balance = balance + $1 WHERE id = $2")
        .bind(delta)
        .bind(player_id)
        .execute(pool)
        .await?;
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════
//  COMPANIES
// ═══════════════════════════════════════════════════════════════════

/// Load all companies — used by the game loop tick.
pub async fn get_all_companies_for_tick(
    pool: &PgPool,
) -> Result<Vec<Company>, sqlx::Error> {
    let rows = sqlx::query_as!(
        Company,
        r#"SELECT id, owner_id, name, treasury, workers, worker_capacity,
                  factories, tech_level, created_at
           FROM companies"#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn create_company(
    pool: &PgPool,
    company: &Company,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO companies (id, owner_id, name, treasury, workers, worker_capacity, factories, tech_level, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"
    )
    .bind(company.id)
    .bind(company.owner_id)
    .bind(&company.name)
    .bind(company.treasury)
    .bind(company.workers)
    .bind(company.worker_capacity)
    .bind(company.factories)
    .bind(company.tech_level)
    .bind(company.created_at)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_company(pool: &PgPool, id: Uuid) -> Result<Option<Company>, sqlx::Error> {
    let row = sqlx::query_as!(
        Company,
        r#"SELECT id, owner_id, name, treasury, workers, worker_capacity,
                  factories, tech_level, created_at
           FROM companies WHERE id = $1"#,
        id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn get_companies_by_owner(
    pool: &PgPool,
    owner_id: Uuid,
) -> Result<Vec<Company>, sqlx::Error> {
    let rows = sqlx::query_as!(
        Company,
        r#"SELECT id, owner_id, name, treasury, workers, worker_capacity,
                  factories, tech_level, created_at
           FROM companies WHERE owner_id = $1"#,
        owner_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn update_company_treasury(
    pool: &PgPool,
    company_id: Uuid,
    delta: Decimal,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE companies SET treasury = treasury + $1 WHERE id = $2")
        .bind(delta)
        .bind(company_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn update_company_workers(
    pool: &PgPool,
    company_id: Uuid,
    workers: i32,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE companies SET workers = $1 WHERE id = $2")
        .bind(workers)
        .bind(company_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn update_company_factories(
    pool: &PgPool,
    company_id: Uuid,
    factories: i32,
    worker_capacity: i32,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE companies SET factories = $1, worker_capacity = $2 WHERE id = $3"
    )
    .bind(factories)
    .bind(worker_capacity)
    .bind(company_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn increment_tech_level(
    pool: &PgPool,
    company_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE companies SET tech_level = tech_level + 1 WHERE id = $1")
        .bind(company_id)
        .execute(pool)
        .await?;
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════
//  RESOURCES & INVENTORY
// ═══════════════════════════════════════════════════════════════════

pub async fn get_all_resources(pool: &PgPool) -> Result<Vec<Resource>, sqlx::Error> {
    let rows = sqlx::query_as!(
        Resource,
        r#"SELECT id, slug, name,
                  category as "category: ResourceCategory",
                  base_price, spawn_rate
           FROM resources"#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn get_resource_by_slug(
    pool: &PgPool,
    slug: &str,
) -> Result<Option<Resource>, sqlx::Error> {
    let row = sqlx::query_as!(
        Resource,
        r#"SELECT id, slug, name,
                  category as "category: ResourceCategory",
                  base_price, spawn_rate
           FROM resources WHERE slug = $1"#,
        slug
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn get_inventory(
    pool: &PgPool,
    owner_id: Uuid,
) -> Result<Vec<Inventory>, sqlx::Error> {
    let rows = sqlx::query_as!(
        Inventory,
        r#"SELECT id, owner_id, resource_id, quantity
           FROM inventories WHERE owner_id = $1"#,
        owner_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn upsert_inventory(
    pool: &PgPool,
    owner_id: Uuid,
    resource_id: Uuid,
    delta: Decimal,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO inventories (id, owner_id, resource_id, quantity)
           VALUES ($1, $2, $3, $4)
           ON CONFLICT (owner_id, resource_id)
           DO UPDATE SET quantity = inventories.quantity + $4"#,
    )
    .bind(Uuid::new_v4())
    .bind(owner_id)
    .bind(resource_id)
    .bind(delta)
    .execute(pool)
    .await?;
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════
//  MARKETS
// ═══════════════════════════════════════════════════════════════════

pub async fn get_all_markets(pool: &PgPool) -> Result<Vec<Market>, sqlx::Error> {
    let rows = sqlx::query_as!(
        Market,
        r#"SELECT id, resource_id, last_price, ema_price,
                  total_supply, total_demand, total_volume, updated_at
           FROM markets"#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn get_market_by_resource(
    pool: &PgPool,
    resource_id: Uuid,
) -> Result<Option<Market>, sqlx::Error> {
    let row = sqlx::query_as!(
        Market,
        r#"SELECT id, resource_id, last_price, ema_price,
                  total_supply, total_demand, total_volume, updated_at
           FROM markets WHERE resource_id = $1"#,
        resource_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn update_market(pool: &PgPool, market: &Market) -> Result<(), sqlx::Error> {
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
    .execute(pool)
    .await?;
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════
//  TRADE ORDERS
// ═══════════════════════════════════════════════════════════════════

pub async fn insert_trade_order(pool: &PgPool, order: &TradeOrder) -> Result<(), sqlx::Error> {
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
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_open_orders_by_resource(
    pool: &PgPool,
    resource_id: Uuid,
    order_type: &str,
) -> Result<Vec<TradeOrder>, sqlx::Error> {
    let rows = sqlx::query_as!(
        TradeOrder,
        r#"SELECT id, player_id, company_id, resource_id,
                  order_type as "order_type: OrderType",
                  price, quantity, original_quantity,
                  status as "status: OrderStatus",
                  created_at
           FROM trade_orders
           WHERE resource_id = $1 AND order_type = $2 AND status IN ('open', 'partially_filled')
           ORDER BY created_at ASC"#,
        resource_id,
        order_type
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn update_order_status(
    pool: &PgPool,
    order_id: Uuid,
    quantity: Decimal,
    status: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE trade_orders SET quantity = $1, status = $2 WHERE id = $3"
    )
    .bind(quantity)
    .bind(status)
    .bind(order_id)
    .execute(pool)
    .await?;
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════
//  TRANSACTIONS
// ═══════════════════════════════════════════════════════════════════

pub async fn insert_transaction(pool: &PgPool, txn: &Transaction) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO transactions
           (id, buy_order_id, sell_order_id, resource_id, buyer_id, seller_id, price, quantity, total_value, executed_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)"#
    )
    .bind(txn.id)
    .bind(txn.buy_order_id)
    .bind(txn.sell_order_id)
    .bind(txn.resource_id)
    .bind(txn.buyer_id)
    .bind(txn.seller_id)
    .bind(txn.price)
    .bind(txn.quantity)
    .bind(txn.total_value)
    .bind(txn.executed_at)
    .execute(pool)
    .await?;
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════
//  RECIPES & PRODUCTION
// ═══════════════════════════════════════════════════════════════════

pub async fn get_all_recipes(pool: &PgPool) -> Result<Vec<Recipe>, sqlx::Error> {
    // Load recipes with their items in two queries, then assemble.
    let raw_recipes: Vec<RawRecipe> = sqlx::query_as!(
        RawRecipe,
        "SELECT id, slug, name, ticks_required, min_tech_level, workers_required FROM recipes"
    )
    .fetch_all(pool)
    .await?;

    let items: Vec<RawRecipeItem> = sqlx::query_as!(
        RawRecipeItem,
        "SELECT id, recipe_id, resource_id, resource_slug, quantity, direction FROM recipe_items"
    )
    .fetch_all(pool)
    .await?;

    let recipes = raw_recipes
        .into_iter()
        .map(|r| {
            let inputs = items
                .iter()
                .filter(|i| i.recipe_id == r.id && i.direction == "input")
                .map(|i| RecipeItem {
                    resource_id: i.resource_id,
                    resource_slug: i.resource_slug.clone(),
                    quantity: i.quantity,
                })
                .collect();
            let outputs = items
                .iter()
                .filter(|i| i.recipe_id == r.id && i.direction == "output")
                .map(|i| RecipeItem {
                    resource_id: i.resource_id,
                    resource_slug: i.resource_slug.clone(),
                    quantity: i.quantity,
                })
                .collect();
            Recipe {
                id: r.id,
                slug: r.slug,
                name: r.name,
                inputs,
                outputs,
                ticks_required: r.ticks_required,
                min_tech_level: r.min_tech_level,
                workers_required: r.workers_required,
            }
        })
        .collect();

    Ok(recipes)
}

// Internal helper structs for query_as! mapping.
#[derive(Debug)]
struct RawRecipe {
    id: Uuid,
    slug: String,
    name: String,
    ticks_required: i32,
    min_tech_level: i32,
    workers_required: i32,
}

#[derive(Debug)]
struct RawRecipeItem {
    id: Uuid,
    recipe_id: Uuid,
    resource_id: Uuid,
    resource_slug: String,
    quantity: Decimal,
    direction: String,
}

pub async fn get_running_jobs(pool: &PgPool) -> Result<Vec<ProductionJob>, sqlx::Error> {
    let rows = sqlx::query_as!(
        ProductionJob,
        r#"SELECT id, company_id, recipe_id, batch_size, ticks_remaining,
                  status as "status: ProductionStatus"
           FROM production_jobs WHERE status = 'running'"#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn insert_production_job(pool: &PgPool, job: &ProductionJob) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO production_jobs (id, company_id, recipe_id, batch_size, ticks_remaining, status)
           VALUES ($1, $2, $3, $4, $5, $6)"#
    )
    .bind(job.id)
    .bind(job.company_id)
    .bind(job.recipe_id)
    .bind(job.batch_size)
    .bind(job.ticks_remaining)
    .bind(format!("{:?}", job.status).to_lowercase())
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn update_production_job(pool: &PgPool, job: &ProductionJob) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE production_jobs SET ticks_remaining = $1, status = $2 WHERE id = $3"
    )
    .bind(job.ticks_remaining)
    .bind(format!("{:?}", job.status).to_lowercase())
    .bind(job.id)
    .execute(pool)
    .await?;
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════
//  MARKET SNAPSHOTS
// ═══════════════════════════════════════════════════════════════════

pub async fn insert_snapshot(pool: &PgPool, snap: &MarketSnapshot) -> Result<(), sqlx::Error> {
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
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_price_history(
    pool: &PgPool,
    resource_id: Uuid,
    limit: i64,
) -> Result<Vec<MarketSnapshot>, sqlx::Error> {
    let rows = sqlx::query_as!(
        MarketSnapshot,
        r#"SELECT id, resource_id, price, volume, supply, demand, recorded_at
           FROM market_snapshots
           WHERE resource_id = $1
           ORDER BY recorded_at DESC
           LIMIT $2"#,
        resource_id,
        limit
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
