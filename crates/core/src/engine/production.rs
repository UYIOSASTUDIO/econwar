use rust_decimal::Decimal;
use uuid::Uuid;

use crate::models::{
    Company, Inventory, ProductionJob, ProductionStatus, Recipe,
};

/// Production engine handles resource transformation.
///
/// When a player starts a production job:
///   1. Validate the company meets tech_level requirements.
///   2. Validate the company has enough workers.
///   3. Deduct input resources from inventory.
///   4. Create a ProductionJob with ticks_remaining = recipe.ticks_required.
///
/// Each simulation tick:
///   - Decrement ticks_remaining for all Running jobs.
///   - When ticks_remaining hits 0, add output resources to inventory.
pub struct ProductionEngine;

/// Errors that can occur during production.
#[derive(Debug, thiserror::Error)]
pub enum ProductionError {
    #[error("Company tech level {have} is below required {need}")]
    InsufficientTech { have: i32, need: i32 },

    #[error("Not enough workers: have {have}, need {need}")]
    InsufficientWorkers { have: i32, need: i32 },

    #[error("Missing resource {slug}: have {have}, need {need}")]
    InsufficientResource {
        slug: String,
        have: Decimal,
        need: Decimal,
    },
}

/// What resources to deduct and add after validation.
#[derive(Debug)]
pub struct ProductionPlan {
    /// (resource_id, quantity_to_deduct)
    pub deductions: Vec<(Uuid, Decimal)>,
    /// Workers allocated to this job.
    pub workers_allocated: i32,
    /// The job to insert.
    pub job: ProductionJob,
}

/// Result of ticking a single production job.
#[derive(Debug)]
pub enum TickResult {
    /// Job still running, N ticks left.
    InProgress(i32),
    /// Job completed — these resources should be added to inventory.
    Completed {
        outputs: Vec<(Uuid, Decimal)>,
        workers_freed: i32,
    },
    /// Job was already completed or cancelled — no action.
    NoOp,
}

impl ProductionEngine {
    /// Validate and plan a new production job.
    /// Does NOT mutate state — returns a plan the caller applies transactionally.
    pub fn plan_production(
        company: &Company,
        recipe: &Recipe,
        batch_size: i32,
        inventories: &[Inventory],
    ) -> Result<ProductionPlan, ProductionError> {
        // 1. Check tech level.
        if company.tech_level < recipe.min_tech_level {
            return Err(ProductionError::InsufficientTech {
                have: company.tech_level,
                need: recipe.min_tech_level,
            });
        }

        // 2. Check workers.
        let workers_needed = recipe.workers_required * batch_size;
        let available_workers = company.worker_capacity - company.workers;
        if available_workers < workers_needed {
            return Err(ProductionError::InsufficientWorkers {
                have: available_workers,
                need: workers_needed,
            });
        }

        // 3. Check and plan resource deductions.
        let mut deductions = Vec::new();
        for input in &recipe.inputs {
            let needed = input.quantity * Decimal::from(batch_size);
            let held = inventories
                .iter()
                .filter(|inv| inv.resource_id == input.resource_id)
                .map(|inv| inv.quantity)
                .sum::<Decimal>();

            if held < needed {
                return Err(ProductionError::InsufficientResource {
                    slug: input.resource_slug.clone(),
                    have: held,
                    need: needed,
                });
            }
            deductions.push((input.resource_id, needed));
        }

        // 4. Build the job.
        let job = ProductionJob {
            id: Uuid::new_v4(),
            company_id: company.id,
            recipe_id: recipe.id,
            batch_size,
            ticks_remaining: recipe.ticks_required,
            status: ProductionStatus::Running,
        };

        Ok(ProductionPlan {
            deductions,
            workers_allocated: workers_needed,
            job,
        })
    }

    /// Advance a production job by one tick.
    pub fn tick_job(job: &mut ProductionJob, recipe: &Recipe) -> TickResult {
        match job.status {
            ProductionStatus::Running => {
                job.ticks_remaining -= 1;
                if job.ticks_remaining <= 0 {
                    job.status = ProductionStatus::Completed;
                    let outputs = recipe
                        .outputs
                        .iter()
                        .map(|o| (o.resource_id, o.quantity * Decimal::from(job.batch_size)))
                        .collect();
                    TickResult::Completed {
                        outputs,
                        workers_freed: recipe.workers_required * job.batch_size,
                    }
                } else {
                    TickResult::InProgress(job.ticks_remaining)
                }
            }
            _ => TickResult::NoOp,
        }
    }
}
