//! Background economic simulation loop (In-Memory Actor).
//!
//! Runs on a dedicated Tokio task. Every `tick_interval_ms` milliseconds:
//!   1. Processes incoming commands from the API (REST/WebSocket).
//!   2. Runs the economic engine tick strictly in memory.
//!   3. Dispatches side-effects (Deltas) to the DbWriter via an MPSC channel.
//!   4. Broadcasts market updates to WebSocket clients.

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

use econwar_core::engine::{EconomicEngine, TickEffects};
use econwar_core::models::{
    Company, Market, ProductionJob, Recipe, Resource, TradeOrder,
};
use econwar_db::repo;

use crate::state::{AppState, MarketBrief, ServerEvent};

/// Payload für den asynchronen Write-Behind Prozess.
#[derive(Debug)]
pub struct TickBatch {
    pub tick_count: u64,
    pub effects: TickEffects,
}

/// Repräsentiert asynchrone Eingaben von Benutzern, die den Spielstatus verändern.
#[derive(Debug)]
pub enum GameCommand {
    CreateOrder(TradeOrder),
    CancelOrder(uuid::Uuid),
    CreateCompany(Company),
    StartProduction(ProductionJob),
    UpdateCompany(Company),
}

/// Temporäre Struktur für API-Effekte innerhalb eines Ticks
struct ApiEffects {
    new_companies: Vec<Company>,
    updated_companies: Vec<Company>,
    new_jobs: Vec<ProductionJob>,
    new_orders: Vec<TradeOrder>,
    cancelled_orders: Vec<uuid::Uuid>,
}

pub struct GameLoopActor {
    engine: EconomicEngine,
    state: Arc<AppState>,
    db_tx: mpsc::Sender<TickBatch>,
    cmd_rx: mpsc::Receiver<GameCommand>,

    // In-Memory State
    companies: Vec<Company>,
    recipes: Vec<Recipe>,
    resources: Vec<Resource>,
    markets: Vec<Market>,
    jobs: Vec<ProductionJob>,
    buys: Vec<TradeOrder>,
    sells: Vec<TradeOrder>,
}

impl GameLoopActor {
    /// Initialisiert den Actor und lädt den Zustand exakt einmal synchron aus der Datenbank.
    pub async fn new(
        state: Arc<AppState>,
        db_tx: mpsc::Sender<TickBatch>,
        cmd_rx: mpsc::Receiver<GameCommand>,
    ) -> anyhow::Result<Self> {
        let pool = &state.db;

        tracing::info!("Initializing GameLoopActor: Loading state from database...");

        let companies = repo::get_all_companies_for_tick(pool).await?;
        let recipes = repo::get_all_recipes(pool).await?;
        let resources = repo::get_all_resources(pool).await?;
        let markets = repo::get_all_markets(pool).await?;
        let jobs = repo::get_running_jobs(pool).await?;

        let mut buys = Vec::new();
        let mut sells = Vec::new();

        for market in &markets {
            let mut market_buys = repo::get_open_orders_by_resource(pool, market.resource_id, "buy").await?;
            let mut market_sells = repo::get_open_orders_by_resource(pool, market.resource_id, "sell").await?;
            buys.append(&mut market_buys);
            sells.append(&mut market_sells);
        }

        tracing::info!(
            "Initialization complete. Loaded {} companies, {} markets, {} open orders.",
            companies.len(),
            markets.len(),
            buys.len() + sells.len()
        );

        Ok(Self {
            engine: EconomicEngine::new(),
            state,
            db_tx,
            cmd_rx,
            companies,
            recipes,
            resources,
            markets,
            jobs,
            buys,
            sells,
        })
    }

    /// Startet die dedizierte In-Memory-Berechnungsschleife.
    pub async fn run(mut self, tick_interval_ms: u64) {
        let mut interval = tokio::time::interval(Duration::from_millis(tick_interval_ms));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        tracing::info!("GameLoopActor running (tick interval {}ms)", tick_interval_ms);

        loop {
            interval.tick().await;

            // 1. Verarbeite alle API-Requests, die seit dem letzten Tick reingekommen sind
            let api_effects = self.process_commands();

            // 2. Führe die Kern-Simulation mit dem aktualisierten RAM-Status durch
            let mut effects = self.engine.tick(
                &self.companies,
                &mut self.jobs,
                &self.recipes,
                &self.resources,
                &mut self.markets,
                &mut self.buys,
                &mut self.sells,
            );

            // 3. Führe API-Effekte und Engine-Effekte zusammen
            effects.new_companies = api_effects.new_companies;
            effects.updated_companies = api_effects.updated_companies;
            effects.new_jobs = api_effects.new_jobs;
            effects.new_orders = api_effects.new_orders;
            effects.cancelled_orders = api_effects.cancelled_orders;

            let tick_count = self.engine.tick_count;
            let batch = TickBatch {
                tick_count,
                effects: effects.clone(),
            };

            // 4. Asynchrones Write-Behind an den DbWriterActor
            if let Err(e) = self.db_tx.try_send(batch) {
                tracing::error!("CRITICAL: Write-behind queue overflow at tick {}: {}", tick_count, e);
            }

            self.broadcast_updates(&effects).await;
        }
    }

    /// Verarbeitet die Warteschlange der API-Befehle synchron im RAM
    fn process_commands(&mut self) -> ApiEffects {
        let mut effects = ApiEffects {
            new_companies: Vec::new(),
            updated_companies: Vec::new(),
            new_jobs: Vec::new(),
            new_orders: Vec::new(),
            cancelled_orders: Vec::new(),
        };

        while let Ok(cmd) = self.cmd_rx.try_recv() {
            match cmd {
                GameCommand::CreateOrder(order) => {
                    let type_str = format!("{:?}", order.order_type).to_lowercase();
                    if type_str == "buy" {
                        self.buys.push(order.clone());
                    } else {
                        self.sells.push(order.clone());
                    }
                    effects.new_orders.push(order);
                }
                GameCommand::CancelOrder(order_id) => {
                    self.buys.retain(|o| o.id != order_id);
                    self.sells.retain(|o| o.id != order_id);
                    effects.cancelled_orders.push(order_id);
                }
                GameCommand::CreateCompany(company) => {
                    self.companies.push(company.clone());
                    effects.new_companies.push(company);
                }
                GameCommand::StartProduction(job) => {
                    self.jobs.push(job.clone());
                    effects.new_jobs.push(job);
                }
                GameCommand::UpdateCompany(updated_company) => {
                    if let Some(c) = self.companies.iter_mut().find(|c| c.id == updated_company.id) {
                        *c = updated_company.clone();
                    }
                    effects.updated_companies.push(updated_company);
                }
            }
        }
        effects
    }

    /// Sendet den aktuellen Marktstatus an verbundene WebSocket-Clients.
    async fn broadcast_updates(&self, effects: &TickEffects) {
        let slugs: std::collections::HashMap<uuid::Uuid, String> = self.resources
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
            let _ = self.state.broadcast_to_redis(ServerEvent::MarketUpdate { markets: briefs }).await;
        }
    }
}