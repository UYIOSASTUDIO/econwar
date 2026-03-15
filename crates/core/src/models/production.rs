use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A production recipe defines how input resources are transformed
/// into output resources.  Recipes are static game data loaded at startup.
///
/// Example: "Electronics" recipe
///   inputs:  [Copper x2, Silicon x1]
///   outputs: [Electronics x1]
///   ticks:   3  (takes 3 simulation ticks to complete)
///   min_tech: 1 (requires tech level >= 1)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recipe {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub inputs: Vec<RecipeItem>,
    pub outputs: Vec<RecipeItem>,
    /// Number of simulation ticks to complete one production cycle.
    pub ticks_required: i32,
    /// Minimum company tech_level needed to use this recipe.
    pub min_tech_level: i32,
    /// Workers consumed per concurrent production run.
    pub workers_required: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecipeItem {
    pub resource_id: Uuid,
    pub resource_slug: String,
    pub quantity: Decimal,
}

/// An active production job running inside a company.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductionJob {
    pub id: Uuid,
    pub company_id: Uuid,
    pub recipe_id: Uuid,
    /// How many concurrent runs of this recipe.
    pub batch_size: i32,
    /// Ticks remaining until this batch completes.
    pub ticks_remaining: i32,
    pub status: ProductionStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "varchar", rename_all = "snake_case")]
pub enum ProductionStatus {
    Running,
    Completed,
    Cancelled,
    /// Paused because the company ran out of workers or inputs mid-cycle.
    Stalled,
}
