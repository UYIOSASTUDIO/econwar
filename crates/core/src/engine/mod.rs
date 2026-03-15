//! Economic Simulation Engine
//!
//! The engine is a pure-logic module that processes game state transitions.
//! It has no I/O — the server feeds it state snapshots and applies the
//! returned deltas to the database.
//!
//! ## Tick Cycle
//! Every N seconds the server triggers `engine.tick()`, which:
//! 1. Advances all active production jobs
//! 2. Pays worker wages (deducting from company treasuries)
//! 3. Spawns raw resources into NPC sell orders (price floor)
//! 4. Records market snapshots for historical data
//! 5. Decays stale orders to prevent order book bloat

mod matching;
mod pricing;
mod production;
mod simulation;

pub use matching::MatchingEngine;
pub use pricing::PricingEngine;
pub use production::ProductionEngine;
pub use simulation::{EconomicEngine, TickEffects};
