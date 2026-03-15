//! Company management endpoints.

use axum::{extract::{Path, State}, http::StatusCode, Json};
use uuid::Uuid;

use econwar_db::repo;
use crate::state::SharedState;

pub async fn list_my_companies(
    State(state): State<SharedState>,
    // In a full implementation, extract player_id from JWT.
    // For MVP, accept it as a query param.
    axum::extract::Query(params): axum::extract::Query<OwnerQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let owner_id = params.owner_id;
    let companies = repo::get_companies_by_owner(&state.db, owner_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(serde_json::to_value(companies).unwrap()))
}

#[derive(serde::Deserialize)]
pub struct OwnerQuery {
    pub owner_id: Uuid,
}

pub async fn get_company(
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let company = repo::get_company(&state.db, id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Company not found".into()))?;

    Ok(Json(serde_json::to_value(company).unwrap()))
}

pub async fn get_inventory(
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let inventory = repo::get_inventory(&state.db, id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(serde_json::to_value(inventory).unwrap()))
}
