//! EconWar Server — main entrypoint.
//!
//! Boots the server in this order:
//!   1. Load configuration from .env
//!   2. Initialize database pool and run migrations
//!   3. Seed initial game data
//!   4. Start the background economic simulation loop
//!   5. Launch the Axum HTTP + WebSocket server

mod api;
mod ws;
mod middleware;
mod game_loop;
mod state;
mod handler;

use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

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

    // ── Shared state ────────────────────────────────────────────────
    let state = Arc::new(AppState::new(pool.clone()));

    // ── Game loop (background task) ─────────────────────────────────
    let loop_state = state.clone();
    tokio::spawn(async move {
        game_loop::run(loop_state, tick_interval_ms).await;
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
