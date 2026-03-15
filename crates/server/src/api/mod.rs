//! REST API routes.
//!
//! All endpoints are JSON-based.  Authentication is via JWT bearer tokens.
//! The API is divided into logical groups:
//!   /api/auth/*       — registration, login
//!   /api/player/*     — player info, balance
//!   /api/company/*    — company management
//!   /api/market/*     — market data, order book
//!   /api/trade/*      — place and cancel orders
//!   /api/production/* — start jobs, view recipes
//!   /api/command      — unified command endpoint (terminal-style)

use axum::{routing::{get, post}, Router};
use crate::state::SharedState;

pub mod auth;
mod command;
mod market;
mod company;

pub fn routes() -> Router<SharedState> {
    Router::new()
        // ── Auth ────────────────────────────────────────────────────
        .route("/api/auth/register", post(auth::register))
        .route("/api/auth/login", post(auth::login))
        // ── Unified command endpoint ────────────────────────────────
        .route("/api/command", post(command::execute_command))
        // ── Market data (public, no auth needed for MVP) ────────────
        .route("/api/markets", get(market::list_markets))
        .route("/api/markets/:slug", get(market::get_market))
        .route("/api/markets/:slug/history", get(market::price_history))
        .route("/api/markets/:slug/orderbook", get(market::order_book))
        // ── Companies ───────────────────────────────────────────────
        .route("/api/companies", get(company::list_my_companies))
        .route("/api/companies/:id", get(company::get_company))
        .route("/api/companies/:id/inventory", get(company::get_inventory))
        // ── Health check ────────────────────────────────────────────
        .route("/api/health", get(|| async { "ok" }))
}
