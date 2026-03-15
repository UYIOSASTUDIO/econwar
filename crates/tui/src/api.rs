//! HTTP client wrapper for talking to the EconWar server.

use anyhow::{anyhow, Result};
use serde_json::Value;
use uuid::Uuid;

use crate::app::{CompanyInfo, InventoryItem, MarketRow};

pub async fn register(base: &str, username: &str, password: &str) -> Result<(String, Uuid, String)> {
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{base}/api/auth/register"))
        .json(&serde_json::json!({"username": username, "password": password}))
        .send().await?;
    if !resp.status().is_success() {
        let text = resp.text().await?;
        return Err(anyhow!("Registration failed: {text}"));
    }
    let body: Value = resp.json().await?;
    let token = body["token"].as_str().unwrap_or("").to_string();
    let player_id = body["player_id"].as_str()
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or_else(|| anyhow!("Invalid player_id"))?;
    let name = body["username"].as_str().unwrap_or("").to_string();
    Ok((token, player_id, name))
}

pub async fn login(base: &str, username: &str, password: &str) -> Result<(String, Uuid, String)> {
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{base}/api/auth/login"))
        .json(&serde_json::json!({"username": username, "password": password}))
        .send().await?;
    if !resp.status().is_success() {
        let text = resp.text().await?;
        return Err(anyhow!("Login failed: {text}"));
    }
    let body: Value = resp.json().await?;
    let token = body["token"].as_str().unwrap_or("").to_string();
    let player_id = body["player_id"].as_str()
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or_else(|| anyhow!("Invalid player_id"))?;
    let name = body["username"].as_str().unwrap_or("").to_string();
    Ok((token, player_id, name))
}

pub async fn send_command(base: &str, player_id: Uuid, command: Value) -> Result<Value> {
    let client = reqwest::Client::new();
    let mut payload = command;
    payload["player_id"] = serde_json::json!(player_id.to_string());
    let resp = client
        .post(format!("{base}/api/command"))
        .json(&payload)
        .send().await?;
    let body: Value = resp.json().await?;
    Ok(body)
}

pub async fn get_markets(base: &str) -> Result<Vec<MarketRow>> {
    let client = reqwest::Client::new();
    let resp = client.get(format!("{base}/api/markets")).send().await?;
    let body: Vec<Value> = resp.json().await?;
    let rows = body.iter().map(|m| MarketRow {
        slug: m["slug"].as_str().unwrap_or("-").to_string(),
        name: m["name"].as_str().unwrap_or("-").to_string(),
        price: m["last_price"].as_str().unwrap_or("0").to_string(),
        ema: m["ema_price"].as_str().unwrap_or("0").to_string(),
        supply: m["total_supply"].as_str().unwrap_or("0").to_string(),
        demand: m["total_demand"].as_str().unwrap_or("0").to_string(),
    }).collect();
    Ok(rows)
}

pub async fn get_company(base: &str, company_id: Uuid) -> Result<CompanyInfo> {
    let client = reqwest::Client::new();
    let resp = client.get(format!("{base}/api/companies/{company_id}")).send().await?;
    let c: Value = resp.json().await?;
    Ok(CompanyInfo {
        id: c["id"].as_str().unwrap_or("-").to_string(),
        name: c["name"].as_str().unwrap_or("-").to_string(),
        treasury: c["treasury"].as_str().unwrap_or("0").to_string(),
        workers: format!("{}", c["workers"].as_i64().unwrap_or(0)),
        capacity: format!("{}", c["worker_capacity"].as_i64().unwrap_or(0)),
        factories: format!("{}", c["factories"].as_i64().unwrap_or(0)),
        tech_level: format!("{}", c["tech_level"].as_i64().unwrap_or(0)),
    })
}

pub async fn get_inventory(base: &str, company_id: Uuid) -> Result<Vec<InventoryItem>> {
    let client = reqwest::Client::new();
    let resp = client.get(format!("{base}/api/companies/{company_id}/inventory")).send().await?;
    let body: Vec<Value> = resp.json().await?;
    let items = body.iter().map(|i| InventoryItem {
        resource: i["resource_id"].as_str().unwrap_or("-").to_string(),
        quantity: i["quantity"].as_str().unwrap_or("0").to_string(),
    }).collect();
    Ok(items)
}
