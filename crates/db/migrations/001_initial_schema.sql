-- EconWar Initial Schema
-- All monetary values use DECIMAL(20,4) for precision without floating-point drift.

-- ── Players ─────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS players (
    id            UUID PRIMARY KEY,
    username      VARCHAR(64) UNIQUE NOT NULL,
    password_hash TEXT NOT NULL,
    balance       DECIMAL(20,4) NOT NULL DEFAULT 100000.0000,
    is_online     BOOLEAN NOT NULL DEFAULT FALSE,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_login    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_players_username ON players(username);

-- ── Resources ───────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS resources (
    id          UUID PRIMARY KEY,
    slug        VARCHAR(64) UNIQUE NOT NULL,
    name        VARCHAR(128) NOT NULL,
    category    VARCHAR(32) NOT NULL,    -- raw_material, component, finished_good, luxury
    base_price  DECIMAL(20,4) NOT NULL,
    spawn_rate  DECIMAL(20,4) NOT NULL DEFAULT 0.0000
);

CREATE INDEX idx_resources_slug ON resources(slug);

-- ── Companies ───────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS companies (
    id              UUID PRIMARY KEY,
    owner_id        UUID NOT NULL REFERENCES players(id),
    name            VARCHAR(128) NOT NULL,
    treasury        DECIMAL(20,4) NOT NULL DEFAULT 0.0000,
    workers         INT NOT NULL DEFAULT 0,
    worker_capacity INT NOT NULL DEFAULT 50,
    factories       INT NOT NULL DEFAULT 1,
    tech_level      INT NOT NULL DEFAULT 0,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_companies_owner ON companies(owner_id);

-- ── Inventories ─────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS inventories (
    id          UUID PRIMARY KEY,
    owner_id    UUID NOT NULL,   -- company id
    resource_id UUID NOT NULL REFERENCES resources(id),
    quantity    DECIMAL(20,4) NOT NULL DEFAULT 0.0000,
    UNIQUE(owner_id, resource_id)
);

CREATE INDEX idx_inventories_owner ON inventories(owner_id);

-- ── Markets ─────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS markets (
    id            UUID PRIMARY KEY,
    resource_id   UUID UNIQUE NOT NULL REFERENCES resources(id),
    last_price    DECIMAL(20,4) NOT NULL,
    ema_price     DECIMAL(20,4) NOT NULL,
    total_supply  DECIMAL(20,4) NOT NULL DEFAULT 0.0000,
    total_demand  DECIMAL(20,4) NOT NULL DEFAULT 0.0000,
    total_volume  DECIMAL(20,4) NOT NULL DEFAULT 0.0000,
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ── Trade Orders (Order Book) ───────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS trade_orders (
    id                UUID PRIMARY KEY,
    player_id         UUID NOT NULL,
    company_id        UUID NOT NULL,
    resource_id       UUID NOT NULL REFERENCES resources(id),
    order_type        VARCHAR(8) NOT NULL,   -- 'buy' or 'sell'
    price             DECIMAL(20,4) NOT NULL,
    quantity          DECIMAL(20,4) NOT NULL,
    original_quantity DECIMAL(20,4) NOT NULL,
    status            VARCHAR(20) NOT NULL DEFAULT 'open',
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_orders_resource_status ON trade_orders(resource_id, status);
CREATE INDEX idx_orders_player ON trade_orders(player_id);

-- ── Transactions (Executed Trades) ──────────────────────────────────────────
CREATE TABLE IF NOT EXISTS transactions (
    id             UUID PRIMARY KEY,
    buy_order_id   UUID NOT NULL,
    sell_order_id  UUID NOT NULL,
    resource_id    UUID NOT NULL REFERENCES resources(id),
    buyer_id       UUID NOT NULL,
    seller_id      UUID NOT NULL,
    price          DECIMAL(20,4) NOT NULL,
    quantity       DECIMAL(20,4) NOT NULL,
    total_value    DECIMAL(20,4) NOT NULL,
    executed_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_transactions_resource ON transactions(resource_id);
CREATE INDEX idx_transactions_time ON transactions(executed_at DESC);

-- ── Recipes ─────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS recipes (
    id              UUID PRIMARY KEY,
    slug            VARCHAR(64) UNIQUE NOT NULL,
    name            VARCHAR(128) NOT NULL,
    ticks_required  INT NOT NULL,
    min_tech_level  INT NOT NULL DEFAULT 0,
    workers_required INT NOT NULL DEFAULT 5
);

CREATE TABLE IF NOT EXISTS recipe_items (
    id          UUID PRIMARY KEY,
    recipe_id   UUID NOT NULL REFERENCES recipes(id),
    resource_id UUID NOT NULL REFERENCES resources(id),
    resource_slug VARCHAR(64) NOT NULL,
    quantity    DECIMAL(20,4) NOT NULL,
    direction   VARCHAR(8) NOT NULL   -- 'input' or 'output'
);

CREATE INDEX idx_recipe_items_recipe ON recipe_items(recipe_id);

-- ── Production Jobs ─────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS production_jobs (
    id              UUID PRIMARY KEY,
    company_id      UUID NOT NULL REFERENCES companies(id),
    recipe_id       UUID NOT NULL REFERENCES recipes(id),
    batch_size      INT NOT NULL DEFAULT 1,
    ticks_remaining INT NOT NULL,
    status          VARCHAR(20) NOT NULL DEFAULT 'running'
);

CREATE INDEX idx_jobs_company ON production_jobs(company_id);
CREATE INDEX idx_jobs_status ON production_jobs(status);

-- ── Market Snapshots (Price History) ────────────────────────────────────────
CREATE TABLE IF NOT EXISTS market_snapshots (
    id          UUID PRIMARY KEY,
    resource_id UUID NOT NULL REFERENCES resources(id),
    price       DECIMAL(20,4) NOT NULL,
    volume      DECIMAL(20,4) NOT NULL,
    supply      DECIMAL(20,4) NOT NULL,
    demand      DECIMAL(20,4) NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_snapshots_resource_time ON market_snapshots(resource_id, recorded_at DESC);

-- ── Chat Messages ───────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS chat_messages (
    id         UUID PRIMARY KEY,
    player_id  UUID NOT NULL REFERENCES players(id),
    username   VARCHAR(64) NOT NULL,
    message    TEXT NOT NULL,
    sent_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_chat_time ON chat_messages(sent_at DESC);
