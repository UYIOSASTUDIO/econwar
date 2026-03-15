//! EconWar Core Library
//!
//! Contains all shared types, game models, the economic simulation engine,
//! and command definitions. This crate has zero I/O dependencies — it is
//! purely computational and can be tested in isolation.

pub mod models;
pub mod engine;
pub mod commands;

// Re-export key types at crate root for ergonomic imports.
pub use models::*;
pub use engine::EconomicEngine;
pub use commands::{GameCommand, CommandResult};
