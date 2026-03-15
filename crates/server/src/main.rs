//! EconWar Server — main entrypoint.

mod api;
mod ws;
mod middleware;
mod game_loop;
mod state;
mod handler;
mod db_writer; // <-- Unser neuer DB Writer Actor

use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;
use bb8_redis::RedisConnectionManager;

use state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ── Config ──────────────────────────────────────────────────────
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            EnvFilter::new("econwar_server=debug,econwar_core=debug,econwar_db=info,tower_http=debug")
        }))
        .init();

    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");
    let host = std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into());
    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "8080".into())
        .parse()?;
    let tick_interval_ms: u64 = std::env::var("TICK_INTERVAL_MS")
        .unwrap_or_else(|_| "5000".into())
        .parse()?;

    // ── Database ────────────────────────────────────────────────────
    let pool = econwar_db::create_pool(&database_url).await?;
    econwar_db::run_migrations(&pool).await?;
    econwar_db::seed::seed_all(&pool).await?;

    // ── Redis ───────────────────────────────────────────────────────
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://127.0.0.1:6379".into());
    let redis_manager = RedisConnectionManager::new(redis_url.clone())?;
    let redis_pool = bb8_redis::bb8::Pool::builder()
        .max_size(15)
        .build(redis_manager)
        .await?;

    // ── Actor System & Channels ─────────────────────────────────────
    let (batch_tx, batch_rx) = tokio::sync::mpsc::channel::<game_loop::TickBatch>(100);
    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel::<game_loop::GameCommand>(1024);

    // ── Shared state ────────────────────────────────────────────────
    let state = Arc::new(AppState::new(pool.clone(), redis_pool, cmd_tx));

    // 1. Redis Subscriber Task
    let redis_sub_url = redis_url;
    let local_tx = state.local_ws_tx.clone();
    tokio::spawn(async move {
        ws::run_redis_subscriber(redis_sub_url, local_tx).await;
    });

    // 2. Database Writer Task
    let db_writer_state = state.clone();
    tokio::spawn(async move {
        let db_writer = db_writer::DbWriterActor::new(db_writer_state, batch_rx);
        db_writer.run().await;
    });

    // 3. In-Memory Game Loop Task
    let loop_state = state.clone();
    tokio::spawn(async move {
        match game_loop::GameLoopActor::new(loop_state, batch_tx, cmd_rx).await {
            Ok(actor) => actor.run(tick_interval_ms).await,
            Err(e) => tracing::error!("CRITICAL: Failed to initialize GameLoopActor: {}", e),
        }
    });

    // ── Routes ──────────────────────────────────────────────────────
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .merge(api::routes())
        .merge(ws::routes())
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    // ── Start server ────────────────────────────────────────────────
    let addr: SocketAddr = format!("{host}:{port}").parse()?;
    tracing::info!("EconWar server listening on {addr}");
    tracing::info!("Tick interval: {tick_interval_ms}ms");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}