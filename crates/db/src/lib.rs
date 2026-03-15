//! Database layer for EconWar.
//!
//! Uses SQLx with PostgreSQL.  All queries are async and use the
//! connection pool.  This crate owns the schema (via migrations)
//! and provides repository functions grouped by domain.

pub mod repo;
pub mod seed;

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

/// Initialize the database connection pool.
pub async fn create_pool(database_url: &str) -> Result<PgPool, sqlx::Error> {
    let pool = PgPoolOptions::new()
        .max_connections(20)
        .connect(database_url)
        .await?;

    tracing::info!("Database pool created (max_connections=20)");
    Ok(pool)
}

/// Run all pending migrations.
pub async fn run_migrations(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await?;
    tracing::info!("Database migrations applied");
    Ok(())
}
