//! Unified command endpoint.
//!
//! Players can issue any game command through POST /api/command.
//! This is the terminal-style interface: the frontend sends a
//! `GameCommand` JSON payload and receives a `CommandResult`.

use axum::{extract::State, http::StatusCode, Json};
use chrono::Utc;
use rust_decimal::Decimal;
use uuid::Uuid;

use econwar_core::commands::{CommandResult, GameCommand};
use econwar_core::models::*;
use econwar_db::repo;
use crate::state::{SharedState, ServerEvent};
use crate::game_loop::GameCommand as LoopCommand; // Alias, um Namenskonflikte zu vermeiden

/// Request payload: the command plus the acting player.
#[derive(serde::Deserialize)]
pub struct CommandRequest {
    pub player_id: Uuid,
    #[serde(flatten)]
    pub command: GameCommand,
}

pub async fn execute_command(
    State(state): State<SharedState>,
    Json(req): Json<CommandRequest>,
) -> Result<Json<CommandResult>, (StatusCode, String)> {
    let result = handle_command(&state, req.player_id, req.command).await;
    Ok(Json(result))
}

async fn handle_command(
    state: &SharedState,
    player_id: Uuid,
    cmd: GameCommand,
) -> CommandResult {
    match cmd {
        // ── Create Company ──────────────────────────────────────────
        // TODO (Architektur): Muss zukünftig in den LoopCommand Channel migriert werden
        GameCommand::CreateCompany { name } => {
            let company = Company {
                id: Uuid::new_v4(),
                owner_id: player_id,
                name: name.clone(),
                treasury: Decimal::ZERO,
                workers: 0,
                worker_capacity: Company::WORKERS_PER_FACTORY,
                factories: 1,
                tech_level: 0,
                created_at: Utc::now(),
            };
            match repo::create_company(&state.db, &company).await {
                Ok(_) => CommandResult::ok(
                    format!("Company '{}' created", name),
                    Some(serde_json::to_value(&company).unwrap()),
                ),
                Err(e) => CommandResult::err(format!("Failed to create company: {e}")),
            }
        }

        // ── Fund Company ────────────────────────────────────────────
        // TODO (Architektur): Muss zukünftig in den LoopCommand Channel migriert werden
        GameCommand::FundCompany { company_id, amount } => {
            if let Err(e) = repo::update_player_balance(&state.db, player_id, -amount).await {
                return CommandResult::err(format!("Failed: {e}"));
            }
            if let Err(e) = repo::update_company_treasury(&state.db, company_id, amount).await {
                return CommandResult::err(format!("Failed: {e}"));
            }
            CommandResult::ok(format!("Funded company with {amount}"), None)
        }

        // ── Hire Workers ────────────────────────────────────────────
        // TODO (Architektur): Muss zukünftig in den LoopCommand Channel migriert werden
        GameCommand::HireWorkers { company_id, count } => {
            let company = match repo::get_company(&state.db, company_id).await {
                Ok(Some(c)) => c,
                _ => return CommandResult::err("Company not found"),
            };
            let new_total = company.workers + count;
            if new_total > company.worker_capacity {
                return CommandResult::err(format!(
                    "Cannot hire {count}: capacity is {}, currently have {}",
                    company.worker_capacity, company.workers
                ));
            }
            let hire_cost = Decimal::from(500) * Decimal::from(count);
            if company.treasury < hire_cost {
                return CommandResult::err(format!(
                    "Insufficient treasury: need {hire_cost}, have {}",
                    company.treasury
                ));
            }
            let _ = repo::update_company_treasury(&state.db, company_id, -hire_cost).await;
            let _ = repo::update_company_workers(&state.db, company_id, new_total).await;
            CommandResult::ok(format!("Hired {count} workers (total: {new_total})"), None)
        }

        // ── Build Factory ───────────────────────────────────────────
        // TODO (Architektur): Muss zukünftig in den LoopCommand Channel migriert werden
        GameCommand::BuildFactory { company_id } => {
            let mut company = match repo::get_company(&state.db, company_id).await {
                Ok(Some(c)) => c,
                _ => return CommandResult::err("Company not found"),
            };
            let cost = company.next_factory_cost();
            if company.treasury < cost {
                return CommandResult::err(format!(
                    "Insufficient treasury: need {cost}, have {}",
                    company.treasury
                ));
            }
            company.factories += 1;
            company.recalculate_capacity();
            let _ = repo::update_company_treasury(&state.db, company_id, -cost).await;
            let _ = repo::update_company_factories(
                &state.db, company_id, company.factories, company.worker_capacity
            ).await;
            CommandResult::ok(
                format!("Factory built! Total: {}. New capacity: {} workers",
                        company.factories, company.worker_capacity),
                None,
            )
        }

        // ── Research Technology ──────────────────────────────────────
        // TODO (Architektur): Muss zukünftig in den LoopCommand Channel migriert werden
        GameCommand::ResearchTechnology { company_id } => {
            let company = match repo::get_company(&state.db, company_id).await {
                Ok(Some(c)) => c,
                _ => return CommandResult::err("Company not found"),
            };
            let cost = Decimal::from(25_000) * Decimal::from((company.tech_level + 1).pow(2));
            if company.treasury < cost {
                return CommandResult::err(format!(
                    "Insufficient treasury: need {cost}, have {}",
                    company.treasury
                ));
            }
            let _ = repo::update_company_treasury(&state.db, company_id, -cost).await;
            let _ = repo::increment_tech_level(&state.db, company_id).await;
            CommandResult::ok(
                format!("Research complete! Tech level: {}", company.tech_level + 1),
                None,
            )
        }

        // ── Buy Resource (Event-Driven) ─────────────────────────────
        GameCommand::BuyResource { company_id, resource_slug, quantity, max_price } => {
            let resource = match repo::get_resource_by_slug(&state.db, &resource_slug).await {
                Ok(Some(r)) => r,
                _ => return CommandResult::err(format!("Unknown resource: {resource_slug}")),
            };
            let total_cost = max_price * quantity;

            // Read-Only Check: Hat das Unternehmen genug Geld?
            let company = match repo::get_company(&state.db, company_id).await {
                Ok(Some(c)) => c,
                _ => return CommandResult::err("Company not found"),
            };

            // In einer perfekten Event-Sourcing Welt wird der Kontostand erst in der Engine
            // geblockt (Escrow). Für den Moment belassen wir die Vorab-Validierung hier.
            if company.treasury < total_cost {
                return CommandResult::err(format!(
                    "Insufficient treasury: need {total_cost}, have {}",
                    company.treasury
                ));
            }

            let order = TradeOrder {
                id: Uuid::new_v4(),
                player_id,
                company_id,
                resource_id: resource.id,
                order_type: OrderType::Buy,
                price: max_price,
                quantity,
                original_quantity: quantity,
                status: OrderStatus::Open,
                created_at: Utc::now(),
            };

            // Non-blocking Push in die In-Memory Engine
            if let Err(e) = state.cmd_tx.send(LoopCommand::CreateOrder(order.clone())).await {
                tracing::error!("Engine overload - failed to route Buy order: {}", e);
                return CommandResult::err("Market engine is currently overloaded.");
            }

            CommandResult::ok(
                format!("Buy order placed: {quantity} {resource_slug} @ {max_price}. Awaiting execution."),
                Some(serde_json::to_value(&order).unwrap()),
            )
        }

        // ── Sell Resource (Event-Driven) ─────────────────────────────
        GameCommand::SellResource { company_id, resource_slug, quantity, min_price } => {
            let resource = match repo::get_resource_by_slug(&state.db, &resource_slug).await {
                Ok(Some(r)) => r,
                _ => return CommandResult::err(format!("Unknown resource: {resource_slug}")),
            };

            // Read-Only Check: Ist die Ressource im Inventar?
            let inventory = repo::get_inventory(&state.db, company_id).await.unwrap_or_default();
            let held: Decimal = inventory
                .iter()
                .filter(|i| i.resource_id == resource.id)
                .map(|i| i.quantity)
                .sum();

            if held < quantity {
                return CommandResult::err(format!(
                    "Insufficient inventory: have {held}, trying to sell {quantity}"
                ));
            }

            let order = TradeOrder {
                id: Uuid::new_v4(),
                player_id,
                company_id,
                resource_id: resource.id,
                order_type: OrderType::Sell,
                price: min_price,
                quantity,
                original_quantity: quantity,
                status: OrderStatus::Open,
                created_at: Utc::now(),
            };

            // Non-blocking Push in die In-Memory Engine
            if let Err(e) = state.cmd_tx.send(LoopCommand::CreateOrder(order.clone())).await {
                tracing::error!("Engine overload - failed to route Sell order: {}", e);
                return CommandResult::err("Market engine is currently overloaded.");
            }

            CommandResult::ok(
                format!("Sell order placed: {quantity} {resource_slug} @ {min_price}. Awaiting execution."),
                Some(serde_json::to_value(&order).unwrap()),
            )
        }

        // ── Scan Market (Read-Only) ──────────────────────────────────
        GameCommand::ScanMarket { resource_slug } => {
            let resource = match repo::get_resource_by_slug(&state.db, &resource_slug).await {
                Ok(Some(r)) => r,
                _ => return CommandResult::err(format!("Unknown resource: {resource_slug}")),
            };
            let market = match repo::get_market_by_resource(&state.db, resource.id).await {
                Ok(Some(m)) => m,
                _ => return CommandResult::err("Market data unavailable"),
            };
            CommandResult::ok(
                format!("{}: price={} ema={} supply={} demand={}",
                        resource_slug, market.last_price, market.ema_price,
                        market.total_supply, market.total_demand),
                Some(serde_json::to_value(&market).unwrap()),
            )
        }

        // ── Scan All Markets (Read-Only) ────────────────────────────
        GameCommand::ScanAllMarkets => {
            let markets = repo::get_all_markets(&state.db).await.unwrap_or_default();
            let resources = repo::get_all_resources(&state.db).await.unwrap_or_default();
            let data: Vec<serde_json::Value> = markets.iter().filter_map(|m| {
                resources.iter().find(|r| r.id == m.resource_id).map(|r| {
                    serde_json::json!({
                        "slug": r.slug,
                        "name": r.name,
                        "price": m.last_price.to_string(),
                        "ema": m.ema_price.to_string(),
                        "supply": m.total_supply.to_string(),
                        "demand": m.total_demand.to_string(),
                    })
                })
            }).collect();
            CommandResult::ok("Market scan complete", Some(serde_json::to_value(data).unwrap()))
        }

        // ── Start Production ────────────────────────────────────────
        // TODO (Architektur): Muss zukünftig in den LoopCommand Channel migriert werden
        GameCommand::StartProduction { company_id, recipe_slug, batch_size } => {
            let recipes = repo::get_all_recipes(&state.db).await.unwrap_or_default();
            let recipe = match recipes.iter().find(|r| r.slug == recipe_slug) {
                Some(r) => r,
                None => return CommandResult::err(format!("Unknown recipe: {recipe_slug}")),
            };
            let company = match repo::get_company(&state.db, company_id).await {
                Ok(Some(c)) => c,
                _ => return CommandResult::err("Company not found"),
            };
            let inventories = repo::get_inventory(&state.db, company_id).await.unwrap_or_default();

            match econwar_core::engine::ProductionEngine::plan_production(
                &company, recipe, batch_size, &inventories,
            ) {
                Ok(plan) => {
                    for (resource_id, qty) in &plan.deductions {
                        let _ = repo::upsert_inventory(&state.db, company_id, *resource_id, -*qty).await;
                    }
                    let new_workers = company.workers + plan.workers_allocated;
                    let _ = repo::update_company_workers(&state.db, company_id, new_workers).await;
                    let _ = repo::insert_production_job(&state.db, &plan.job).await;
                    CommandResult::ok(
                        format!("Production started: {} x{batch_size} ({} ticks)",
                                recipe.name, recipe.ticks_required),
                        Some(serde_json::to_value(&plan.job).unwrap()),
                    )
                }
                Err(e) => CommandResult::err(format!("Cannot start production: {e}")),
            }
        }

        // ── List Recipes (Read-Only) ────────────────────────────────
        GameCommand::ListRecipes => {
            let recipes = repo::get_all_recipes(&state.db).await.unwrap_or_default();
            CommandResult::ok(
                format!("{} recipes available", recipes.len()),
                Some(serde_json::to_value(&recipes).unwrap()),
            )
        }

        // ── View Company (Read-Only) ────────────────────────────────
        GameCommand::ViewCompany { company_id } => {
            match repo::get_company(&state.db, company_id).await {
                Ok(Some(c)) => CommandResult::ok("Company details", Some(serde_json::to_value(&c).unwrap())),
                _ => CommandResult::err("Company not found"),
            }
        }

        // ── View Inventory (Read-Only) ──────────────────────────────
        GameCommand::ViewInventory { company_id } => {
            let inv = repo::get_inventory(&state.db, company_id).await.unwrap_or_default();
            CommandResult::ok(
                format!("{} items in inventory", inv.len()),
                Some(serde_json::to_value(&inv).unwrap()),
            )
        }

        // ── View Balance (Read-Only) ────────────────────────────────
        GameCommand::ViewBalance => {
            match repo::get_player_by_id(&state.db, player_id).await {
                Ok(Some(p)) => CommandResult::ok(
                    format!("Balance: {}", p.balance),
                    Some(serde_json::json!({"balance": p.balance.to_string()})),
                ),
                _ => CommandResult::err("Player not found"),
            }
        }

        // ── List Companies (Read-Only) ──────────────────────────────
        GameCommand::ListCompanies => {
            let companies = repo::get_companies_by_owner(&state.db, player_id)
                .await
                .unwrap_or_default();
            CommandResult::ok(
                format!("{} companies", companies.len()),
                Some(serde_json::to_value(&companies).unwrap()),
            )
        }

        // ── Global Chat (Redis Pub/Sub) ─────────────────────────────
        GameCommand::GlobalChat { message } => {
            let username = match repo::get_player_by_id(&state.db, player_id).await {
                Ok(Some(player)) => player.username,
                _ => "anonymous".to_string(),
            };

            let _ = state.broadcast_to_redis(ServerEvent::ChatMessage {
                username,
                message: message.clone(),
                timestamp: Utc::now().to_rfc3339(),
            }).await;

            CommandResult::ok("Message sent", None)
        }

        // ── Cancel Order (Not implemented) ──────────────────────────
        GameCommand::CancelOrder { .. } => {
            CommandResult::err("Order cancellation is managed by the in-memory engine and is not yet implemented.")
        }

        // ── Catch-all ───────────────────────────────────────────────
        #[allow(unreachable_patterns)]
        _ => CommandResult::err("Command not yet implemented"),
    }
}