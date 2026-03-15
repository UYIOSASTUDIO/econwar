# EconWar — System Architecture & Setup Guide

## 1. System Architecture Overview

```
┌──────────────────────────────────────────────────────────────────┐
│                        CLIENTS                                    │
│  React Dashboard  │  Terminal UI  │  Mobile App  │  Bot API       │
└────────┬─────────────────┬────────────────┬──────────────────────┘
         │ REST (JSON)     │ WebSocket       │ REST
         ▼                 ▼                 ▼
┌──────────────────────────────────────────────────────────────────┐
│                    AXUM HTTP SERVER                                │
│  ┌─────────┐  ┌──────────┐  ┌───────────┐  ┌──────────────┐     │
│  │ Auth    │  │ Command  │  │ Market    │  │ WebSocket    │     │
│  │ Handler │  │ Router   │  │ Endpoints │  │ Handler      │     │
│  └────┬────┘  └────┬─────┘  └─────┬─────┘  └──────┬───────┘     │
│       │            │              │                │              │
│       ▼            ▼              ▼                ▼              │
│  ┌──────────────────────────────────────────────────────────┐    │
│  │              SHARED APP STATE (Arc<AppState>)            │    │
│  │  • PgPool        • broadcast::Sender<ServerEvent>       │    │
│  │  • DashMap<Uuid, String>  (online sessions)             │    │
│  └──────────────────────────────────────────────────────────┘    │
└──────────────────────────────────────────────────────────────────┘
         │                              │
         ▼                              ▼
┌──────────────────┐    ┌──────────────────────────────────────────┐
│   PostgreSQL     │    │         GAME LOOP (Background Task)      │
│   Database       │◄───┤  Every 5s:                               │
│                  │    │   1. Advance production jobs              │
│  • players       │    │   2. Pay wages                           │
│  • companies     │    │   3. Spawn NPC raw materials             │
│  • resources     │    │   4. Run matching engine per market      │
│  • markets       │    │   5. Record snapshots                    │
│  • trade_orders  │    │   6. Broadcast updates via WebSocket     │
│  • transactions  │    │                                          │
│  • recipes       │    │  Uses: EconomicEngine (pure Rust,        │
│  • prod_jobs     │    │        no I/O, fully testable)           │
│  • snapshots     │    └──────────────────────────────────────────┘
└──────────────────┘

```

## 2. Crate Structure

```
econwar/
├── Cargo.toml              # Workspace root
├── .env.example            # Environment config template
├── scripts/
│   └── setup_db.sh         # One-time DB setup
├── docs/
│   └── ARCHITECTURE.md     # This file
└── crates/
    ├── core/               # Pure logic, zero I/O
    │   └── src/
    │       ├── models/     # Player, Company, Resource, Market, etc.
    │       ├── engine/     # EconomicEngine, MatchingEngine, PricingEngine
    │       └── commands/   # GameCommand enum + CommandResult
    ├── db/                 # PostgreSQL via SQLx
    │   ├── migrations/     # SQL migration files
    │   └── src/
    │       ├── repo.rs     # Query functions grouped by entity
    │       └── seed.rs     # Initial game data (resources, recipes)
    └── server/             # Axum HTTP + WebSocket
        └── src/
            ├── main.rs     # Entrypoint: config → DB → game loop → serve
            ├── state.rs    # AppState + ServerEvent + broadcast
            ├── game_loop.rs# Background tick task
            ├── api/        # REST endpoints
            │   ├── auth.rs
            │   ├── command.rs
            │   ├── market.rs
            │   └── company.rs
            ├── ws/         # WebSocket handler
            └── middleware/  # JWT auth extractor
```

## 3. Technology Stack

| Layer           | Technology      | Justification                                    |
|-----------------|-----------------|--------------------------------------------------|
| Language        | Rust            | Zero-cost abstractions, fearless concurrency     |
| HTTP Framework  | Axum 0.7        | Tower-based, great ergonomics, async-native      |
| Database        | PostgreSQL      | ACID transactions, JSON support, proven scale    |
| DB Driver       | SQLx            | Async, compile-time checked queries              |
| Auth            | JWT + Argon2    | Stateless auth + secure password hashing         |
| Realtime        | WebSockets      | Low-latency push for market updates              |
| Concurrency     | Tokio           | Industry-standard async runtime                  |
| Serialization   | Serde + JSON    | Universal, fast, well-supported                  |

## 4. Economic Model

**Prices emerge from player activity, not server formulas.**

The matching engine runs a **continuous double auction** (limit order book):
- Buy orders sorted by price DESC (highest bid first)
- Sell orders sorted by price ASC (lowest ask first)
- When best_bid >= best_ask, a trade executes at the maker's price
- Partial fills are supported

**NPC Price Floor/Ceiling:**
- Raw materials have NPC sellers at 50% of base_price (prevents zero prices)
- If price exceeds 5x base, NPCs flood supply (prevents runaway inflation)

**EMA Smoothing:** Displayed prices use exponential moving average (α=0.1)
to smooth out single-trade noise.

## 5. Production Chain

```
RAW MATERIALS          COMPONENTS              FINISHED GOODS         LUXURY
─────────────          ──────────              ──────────────         ──────
Iron Ore ──────────►  Steel ──────────┐
                                      ├──────► Machines ──────┐
Copper ──────┐                        │                       │
Silicon ─────┴──────► Electronics ────┘                       ├──► Luxury Goods
                                      ┌──────► Vehicles ──────┘
Lithium ─────┐        Battery Pack ───┤
Copper ──────┤                        │
Plastic ─────┘                        │
                                      │
Oil ────────────────► Fuel ───────────┘

Plastic ─────────────────┐
Electronics ─────────────┤
Battery Pack ────────────┴──► Consumer Tech ──────► Luxury Goods
```

## 6. Setup Instructions

### Prerequisites
- Rust (latest stable): `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
- PostgreSQL 14+
- SQLx CLI: `cargo install sqlx-cli --no-default-features --features postgres`

### Step-by-step

```bash
# 1. Clone and enter project
cd econwar

# 2. Copy environment config
cp .env.example .env
# Edit .env with your database credentials

# 3. Create the database
# Option A: Use the setup script
chmod +x scripts/setup_db.sh
./scripts/setup_db.sh

# Option B: Manual
psql -U postgres -c "CREATE USER econwar WITH PASSWORD 'econwar';"
psql -U postgres -c "CREATE DATABASE econwar OWNER econwar;"

# 4. Run migrations
sqlx migrate run --source crates/db/migrations

# 5. Build the project
cargo build --release

# 6. Run the server
cargo run --release --bin econwar-server

# Server will:
#   - Connect to PostgreSQL
#   - Run any pending migrations
#   - Seed initial game data (resources, recipes)
#   - Start the economic simulation loop (every 5s)
#   - Listen on http://0.0.0.0:8080
```

### Verify it works

```bash
# Health check
curl http://localhost:8080/api/health

# Register a player
curl -X POST http://localhost:8080/api/auth/register \
  -H "Content-Type: application/json" \
  -d '{"username": "player1", "password": "secret123"}'

# List all markets
curl http://localhost:8080/api/markets

# Create a company (use player_id from registration)
curl -X POST http://localhost:8080/api/command \
  -H "Content-Type: application/json" \
  -d '{
    "player_id": "<YOUR_PLAYER_ID>",
    "command": "create_company",
    "name": "Acme Corp"
  }'

# Scan a market
curl -X POST http://localhost:8080/api/command \
  -H "Content-Type: application/json" \
  -d '{
    "player_id": "<YOUR_PLAYER_ID>",
    "command": "scan_market",
    "resource_slug": "copper"
  }'
```

## 7. MVP Development Roadmap

### Phase 1: Foundation (Current) ✓
- [x] Workspace structure with 3 crates
- [x] Database schema + migrations
- [x] Player registration + JWT auth
- [x] Company creation and management
- [x] Resource/recipe seed data
- [x] Economic engine (matching, pricing, production)
- [x] Unified command endpoint
- [x] WebSocket real-time broadcast
- [x] Game loop with tick-based simulation

### Phase 2: Core Gameplay
- [ ] Full order cancellation with fund/resource refund
- [ ] Production job status in WS events
- [ ] Player-to-player direct trade offers
- [ ] Company bankruptcy mechanics (negative treasury)
- [ ] Worker wage scaling with tech level
- [ ] Rate limiting per player

### Phase 3: Data & Analytics
- [ ] Price history charting data (OHLCV candles)
- [ ] Trade volume heatmaps
- [ ] Supply/demand trend indicators
- [ ] Player leaderboard (by net worth)
- [ ] Company financial reports

### Phase 4: Advanced Economy
- [ ] Resource decay (perishable goods)
- [ ] Transportation costs between regions
- [ ] Taxation and government mechanics
- [ ] Corporate mergers and acquisitions
- [ ] Market manipulation detection

### Phase 5: Frontend
- [ ] React dashboard with terminal-style command input
- [ ] Real-time market ticker (WebSocket)
- [ ] Interactive order book visualization
- [ ] Production chain builder UI
- [ ] Chat interface

### Phase 6: Scale
- [ ] Horizontal scaling with Redis pub/sub
- [ ] Read replicas for market data queries
- [ ] Connection pooling optimization
- [ ] Load testing with simulated players
- [ ] Kubernetes deployment manifests
