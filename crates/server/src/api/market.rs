//! Market data endpoints — public, no auth required for MVP.

use axum::{extract::{Path, Query, State}, http::StatusCode, Json};
use serde::{Deserialize, Serialize};

use econwar_db::repo;
use crate::state::SharedState;

#[derive(Serialize)]
pub struct MarketView {
    pub slug: String,
    pub name: String,
    pub last_price: String,
    pub ema_price: String,
    pub total_supply: String,
    pub total_demand: String,
    pub total_volume: String,
}

pub async fn list_markets(
    State(state): State<SharedState>,
) -> Result<Json<Vec<MarketView>>, (StatusCode, String)> {
    let markets = repo::get_all_markets(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let resources = repo::get_all_resources(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let views: Vec<MarketView> = markets
        .iter()
        .filter_map(|m| {
            resources.iter().find(|r| r.id == m.resource_id).map(|r| MarketView {
                slug: r.slug.clone(),
                name: r.name.clone(),
                last_price: m.last_price.to_string(),
                ema_price: m.ema_price.to_string(),
                total_supply: m.total_supply.to_string(),
                total_demand: m.total_demand.to_string(),
                total_volume: m.total_volume.to_string(),
            })
        })
        .collect();

    Ok(Json(views))
}

pub async fn get_market(
    State(state): State<SharedState>,
    Path(slug): Path<String>,
) -> Result<Json<MarketView>, (StatusCode, String)> {
    let resource = repo::get_resource_by_slug(&state.db, &slug)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Resource not found".into()))?;

    let market = repo::get_market_by_resource(&state.db, resource.id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Market not found".into()))?;

    Ok(Json(MarketView {
        slug: resource.slug,
        name: resource.name,
        last_price: market.last_price.to_string(),
        ema_price: market.ema_price.to_string(),
        total_supply: market.total_supply.to_string(),
        total_demand: market.total_demand.to_string(),
        total_volume: market.total_volume.to_string(),
    }))
}

#[derive(Deserialize)]
pub struct HistoryParams {
    pub limit: Option<i64>,
}

pub async fn price_history(
    State(state): State<SharedState>,
    Path(slug): Path<String>,
    Query(params): Query<HistoryParams>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let resource = repo::get_resource_by_slug(&state.db, &slug)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Resource not found".into()))?;

    let limit = params.limit.unwrap_or(50);
    let history = repo::get_price_history(&state.db, resource.id, limit)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(serde_json::to_value(history).unwrap()))
}

pub async fn order_book(
    State(state): State<SharedState>,
    Path(slug): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let resource = repo::get_resource_by_slug(&state.db, &slug)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Resource not found".into()))?;

    let buys = repo::get_open_orders_by_resource(&state.db, resource.id, "buy")
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let sells = repo::get_open_orders_by_resource(&state.db, resource.id, "sell")
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(serde_json::json!({
        "resource": slug,
        "bids": buys,
        "asks": sells,
    })))
}
