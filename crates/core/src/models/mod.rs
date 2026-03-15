//! Domain models for the entire game.
//!
//! Every struct here is a pure data type with serde support.
//! No database or network logic lives in this module.

mod player;
mod company;
mod resource;
mod market;
mod production;
mod transaction;

pub use player::*;
pub use company::*;
pub use resource::*;
pub use market::*;
pub use production::*;
pub use transaction::*;
