//! Human-friendly command parser.

use serde_json::json;
use uuid::Uuid;

use crate::api;
use crate::app::App;

pub async fn handle_command(app: &mut App, raw: &str) {
    let raw = raw.trim();
    if raw.is_empty() { return; }
    let tokens = tokenize(raw);
    if tokens.is_empty() { return; }

    let cmd = tokens[0].to_lowercase();
    let args = &tokens[1..];

    match cmd.as_str() {
        "register" => cmd_register(app, args).await,
        "login" => cmd_login(app, args).await,
        "logout" => {
            app.token = None; app.player_id = None; app.username = None;
            app.active_company_id = None; app.active_company = None;
            app.log("Logged out.");
        }
        "create" => cmd_create(app, args).await,
        "fund" => cmd_fund(app, args).await,
        "hire" => cmd_hire(app, args).await,
        "build" => cmd_build(app, args).await,
        "research" => cmd_research(app).await,
        "select" | "use" => cmd_select_company(app, args).await,
        "markets" | "m" => cmd_markets(app).await,
        "scan" => cmd_scan(app, args).await,
        "buy" | "b" => cmd_buy(app, args).await,
        "sell" | "s" => cmd_sell(app, args).await,
        "produce" | "p" => cmd_produce(app, args).await,
        "recipes" | "r" => cmd_recipes(app).await,
        "status" | "st" => cmd_status(app).await,
        "inventory" | "inv" | "i" => cmd_inventory(app).await,
        "balance" | "bal" => cmd_balance(app).await,
        "companies" | "co" => cmd_companies(app).await,
        "help" | "h" | "?" => cmd_help(app),
        "clear" | "cls" => { app.log_messages.clear(); app.log_scroll = 0; }
        "quit" | "exit" => { app.log("Use Ctrl+C or 'q' in normal mode to quit."); }
        _ => { app.log_error(&format!("Unknown command: '{cmd}'. Type 'help' for a list.")); }
    }
}

async fn cmd_register(app: &mut App, args: &[String]) {
    if args.len() < 2 { app.log_error("Usage: register <username> <password>"); return; }
    match api::register(&app.server_url, &args[0], &args[1]).await {
        Ok((token, pid, name)) => {
            app.token = Some(token); app.player_id = Some(pid); app.username = Some(name.clone());
            app.log_success(&format!("Registered and logged in as '{name}'"));
            app.log(&format!("Player ID: {pid}"));
            app.log("Create a company with: create company <name>");
            app.refresh_markets().await;
        }
        Err(e) => app.log_error(&format!("Registration failed: {e}")),
    }
}

async fn cmd_login(app: &mut App, args: &[String]) {
    if args.len() < 2 { app.log_error("Usage: login <username> <password>"); return; }
    match api::login(&app.server_url, &args[0], &args[1]).await {
        Ok((token, pid, name)) => {
            app.token = Some(token); app.player_id = Some(pid); app.username = Some(name.clone());
            app.log_success(&format!("Logged in as '{name}'"));
            cmd_companies(app).await;
            app.refresh_markets().await;
        }
        Err(e) => app.log_error(&format!("Login failed: {e}")),
    }
}

async fn cmd_create(app: &mut App, args: &[String]) {
    if args.is_empty() || args[0].to_lowercase() != "company" || args.len() < 2 {
        app.log_error("Usage: create company <name>"); return;
    }
    let name = args[1..].join(" ");
    let pid = match app.player_id { Some(id) => id, None => { app.log_error("Login first."); return; } };
    let result = api::send_command(&app.server_url, pid, json!({"command": "create_company", "name": name})).await;
    match result {
        Ok(v) => {
            app.log_success(v["message"].as_str().unwrap_or("Company created"));
            if let Some(data) = v.get("data") {
                if let Some(cid_str) = data["id"].as_str() {
                    if let Ok(cid) = Uuid::parse_str(cid_str) {
                        app.active_company_id = Some(cid);
                        app.log(&format!("Auto-selected company: {cid}"));
                        app.refresh_company().await;
                    }
                }
            }
        }
        Err(e) => app.log_error(&format!("{e}")),
    }
}

async fn cmd_fund(app: &mut App, args: &[String]) {
    if args.is_empty() { app.log_error("Usage: fund <amount>"); return; }
    let (pid, cid) = match require_company(app) { Some(v) => v, None => return };
    let result = api::send_command(&app.server_url, pid,
        json!({"command": "fund_company", "company_id": cid.to_string(), "amount": args[0]})).await;
    handle_result(app, result).await;
}

async fn cmd_hire(app: &mut App, args: &[String]) {
    if args.is_empty() { app.log_error("Usage: hire <count>"); return; }
    let count: i32 = match args[0].parse() { Ok(n) => n, Err(_) => { app.log_error("Invalid number."); return; } };
    let (pid, cid) = match require_company(app) { Some(v) => v, None => return };
    let result = api::send_command(&app.server_url, pid,
        json!({"command": "hire_workers", "company_id": cid.to_string(), "count": count})).await;
    handle_result(app, result).await;
}

async fn cmd_build(app: &mut App, args: &[String]) {
    if args.is_empty() || args[0].to_lowercase() != "factory" {
        app.log_error("Usage: build factory"); return;
    }
    let (pid, cid) = match require_company(app) { Some(v) => v, None => return };
    let result = api::send_command(&app.server_url, pid,
        json!({"command": "build_factory", "company_id": cid.to_string()})).await;
    handle_result(app, result).await;
}

async fn cmd_research(app: &mut App) {
    let (pid, cid) = match require_company(app) { Some(v) => v, None => return };
    let result = api::send_command(&app.server_url, pid,
        json!({"command": "research_technology", "company_id": cid.to_string()})).await;
    handle_result(app, result).await;
}

async fn cmd_select_company(app: &mut App, args: &[String]) {
    if args.is_empty() { app.log_error("Usage: select <company_id>"); return; }
    if let Ok(cid) = Uuid::parse_str(&args[0]) {
        app.active_company_id = Some(cid);
        app.log_success(&format!("Selected company: {cid}"));
        app.refresh_company().await;
    } else {
        app.log_error("Invalid company ID. Use 'companies' to list them.");
    }
}

async fn cmd_markets(app: &mut App) {
    match api::get_markets(&app.server_url).await {
        Ok(rows) => {
            if rows.is_empty() { app.log("No market data available."); return; }
            app.log("------------- MARKET OVERVIEW -------------");
            app.log(&format!(" {:<16} {:>10} {:>10} {:>10} {:>10}", "RESOURCE", "PRICE", "EMA", "SUPPLY", "DEMAND"));
            for r in &rows {
                app.log(&format!(" {:<16} {:>10} {:>10} {:>10} {:>10}",
                    r.slug, truncate_decimal(&r.price), truncate_decimal(&r.ema),
                    truncate_decimal(&r.supply), truncate_decimal(&r.demand)));
            }
            app.markets = rows;
        }
        Err(e) => app.log_error(&format!("Failed to fetch markets: {e}")),
    }
}

async fn cmd_scan(app: &mut App, args: &[String]) {
    if args.is_empty() { cmd_markets(app).await; return; }
    let pid = match app.player_id { Some(id) => id, None => { app.log_error("Login first."); return; } };
    let result = api::send_command(&app.server_url, pid,
        json!({"command": "scan_market", "resource_slug": args[0]})).await;
    match result {
        Ok(v) => { app.log_success(v["message"].as_str().unwrap_or("-")); }
        Err(e) => app.log_error(&format!("{e}")),
    }
}

async fn cmd_buy(app: &mut App, args: &[String]) {
    if args.len() < 3 {
        app.log_error("Usage: buy <qty> <resource> <price>");
        app.log_error("  Example: buy 100 copper 40"); return;
    }
    let (pid, cid) = match require_company(app) { Some(v) => v, None => return };
    let qty = &args[0]; let resource = &args[1];
    let price_idx = if args.len() > 3 && args[2] == "@" { 3 } else { 2 };
    let price = &args[price_idx];
    let result = api::send_command(&app.server_url, pid, json!({
        "command": "buy_resource", "company_id": cid.to_string(),
        "resource_slug": resource, "quantity": qty, "max_price": price,
    })).await;
    handle_result(app, result).await;
}

async fn cmd_sell(app: &mut App, args: &[String]) {
    if args.len() < 3 {
        app.log_error("Usage: sell <qty> <resource> <price>");
        app.log_error("  Example: sell 50 steel 120"); return;
    }
    let (pid, cid) = match require_company(app) { Some(v) => v, None => return };
    let qty = &args[0]; let resource = &args[1];
    let price_idx = if args.len() > 3 && args[2] == "@" { 3 } else { 2 };
    let price = &args[price_idx];
    let result = api::send_command(&app.server_url, pid, json!({
        "command": "sell_resource", "company_id": cid.to_string(),
        "resource_slug": resource, "quantity": qty, "min_price": price,
    })).await;
    handle_result(app, result).await;
}

async fn cmd_produce(app: &mut App, args: &[String]) {
    if args.is_empty() {
        app.log_error("Usage: produce <recipe> [quantity]");
        app.log_error("  Example: produce smelt_steel 5"); return;
    }
    let (pid, cid) = match require_company(app) { Some(v) => v, None => return };
    let recipe = &args[0];
    let batch: i32 = if args.len() > 1 { args[1].trim_start_matches('x').parse().unwrap_or(1) } else { 1 };
    let result = api::send_command(&app.server_url, pid, json!({
        "command": "start_production", "company_id": cid.to_string(),
        "recipe_slug": recipe, "batch_size": batch,
    })).await;
    handle_result(app, result).await;
}

async fn cmd_recipes(app: &mut App) {
    let pid = match app.player_id { Some(id) => id, None => { app.log_error("Login first."); return; } };
    let result = api::send_command(&app.server_url, pid, json!({"command": "list_recipes"})).await;
    match result {
        Ok(v) => {
            if let Some(data) = v.get("data") {
                if let Some(recipes) = data.as_array() {
                    app.log("------------- RECIPES -------------");
                    for recipe in recipes {
                        let name = recipe["name"].as_str().unwrap_or("-");
                        let slug = recipe["slug"].as_str().unwrap_or("-");
                        let ticks = recipe["ticks_required"].as_i64().unwrap_or(0);
                        let tech = recipe["min_tech_level"].as_i64().unwrap_or(0);
                        let workers = recipe["workers_required"].as_i64().unwrap_or(0);
                        let inputs: Vec<String> = recipe["inputs"].as_array()
                            .map(|arr| arr.iter().map(|i| format!("{}x{}", i["quantity"].as_str().unwrap_or("?"), i["resource_slug"].as_str().unwrap_or("?"))).collect())
                            .unwrap_or_default();
                        let outputs: Vec<String> = recipe["outputs"].as_array()
                            .map(|arr| arr.iter().map(|i| format!("{}x{}", i["quantity"].as_str().unwrap_or("?"), i["resource_slug"].as_str().unwrap_or("?"))).collect())
                            .unwrap_or_default();
                        app.log(&format!(" [{slug}] {name} | {ticks} ticks | tech:{tech} | workers:{workers}"));
                        app.log(&format!("   {} -> {}", inputs.join(" + "), outputs.join(" + ")));
                    }
                }
            }
        }
        Err(e) => app.log_error(&format!("{e}")),
    }
}

async fn cmd_status(app: &mut App) {
    let uname = app.username.clone();
    if let Some(name) = uname {
        app.log(&format!("Player: {name}"));
    } else { app.log("Not logged in."); return; }
    cmd_balance(app).await;
    if app.active_company_id.is_some() {
        app.refresh_company().await;
        let company = app.active_company.clone();
        if let Some(c) = company {
            app.log("------------- COMPANY STATUS -------------");
            app.log(&format!(" Name:      {}", c.name));
            app.log(&format!(" Treasury:  {} credits", c.treasury));
            app.log(&format!(" Workers:   {} / {}", c.workers, c.capacity));
            app.log(&format!(" Factories: {}", c.factories));
            app.log(&format!(" Tech:      Level {}", c.tech_level));
        }
    } else { app.log("No company selected."); }
}

async fn cmd_inventory(app: &mut App) {
    let (pid, cid) = match require_company(app) { Some(v) => v, None => return };
    let result = api::send_command(&app.server_url, pid,
        json!({"command": "view_inventory", "company_id": cid.to_string()})).await;
    match result {
        Ok(v) => {
            if let Some(data) = v.get("data") {
                if let Some(items) = data.as_array() {
                    if items.is_empty() { app.log("Inventory is empty."); } else {
                        app.log("------------- INVENTORY -------------");
                        for item in items {
                            let rid = item["resource_id"].as_str().unwrap_or("?");
                            let qty = item["quantity"].as_str().unwrap_or("0");
                            app.log(&format!("  {rid}: {qty}"));
                        }
                    }
                }
            }
        }
        Err(e) => app.log_error(&format!("{e}")),
    }
}

async fn cmd_balance(app: &mut App) {
    let pid = match app.player_id { Some(id) => id, None => { app.log_error("Login first."); return; } };
    let result = api::send_command(&app.server_url, pid, json!({"command": "view_balance"})).await;
    match result {
        Ok(v) => { app.log_success(v["message"].as_str().unwrap_or("-")); }
        Err(e) => app.log_error(&format!("{e}")),
    }
}

async fn cmd_companies(app: &mut App) {
    let pid = match app.player_id { Some(id) => id, None => { app.log_error("Login first."); return; } };
    let result = api::send_command(&app.server_url, pid, json!({"command": "list_companies"})).await;
    match result {
        Ok(v) => {
            if let Some(data) = v.get("data") {
                if let Some(companies) = data.as_array() {
                    if companies.is_empty() {
                        app.log("No companies. Create one with: create company <name>");
                    } else {
                        app.log("------------- YOUR COMPANIES -------------");
                        for c in companies {
                            let id = c["id"].as_str().unwrap_or("-");
                            let name = c["name"].as_str().unwrap_or("-");
                            let treasury = c["treasury"].as_str().unwrap_or("0");
                            app.log(&format!("  [{id}] {name} | Treasury: {treasury}"));
                        }
                        app.log("Use 'select <company_id>' to manage a company.");
                        if app.active_company_id.is_none() {
                            if let Some(first) = companies.first() {
                                if let Some(cid_str) = first["id"].as_str() {
                                    if let Ok(cid) = Uuid::parse_str(cid_str) {
                                        app.active_company_id = Some(cid);
                                        app.log_success(&format!("Auto-selected: {}", first["name"].as_str().unwrap_or("?")));
                                        app.refresh_company().await;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Err(e) => app.log_error(&format!("{e}")),
    }
}

fn cmd_help(app: &mut App) {
    app.log("===================================================");
    app.log("                  ECONWAR COMMANDS                  ");
    app.log("===================================================");
    app.log("");
    app.log("  GETTING STARTED");
    app.log("  register <user> <pass>    Create a new account");
    app.log("  login <user> <pass>       Login to existing account");
    app.log("  logout                    Logout");
    app.log("");
    app.log("  COMPANY MANAGEMENT");
    app.log("  create company <name>     Create a new company");
    app.log("  companies (co)            List your companies");
    app.log("  select <id>               Select active company");
    app.log("  fund <amount>             Transfer credits to company");
    app.log("  hire <count>              Hire workers");
    app.log("  build factory             Build a new factory");
    app.log("  research                  Upgrade tech level");
    app.log("");
    app.log("  MARKET & TRADING");
    app.log("  markets (m)               Show all market prices");
    app.log("  scan <resource>           Detailed market info");
    app.log("  buy <qty> <res> <price>   Place buy order");
    app.log("  sell <qty> <res> <price>  Place sell order");
    app.log("    Example: buy 100 copper 40");
    app.log("    Example: sell 50 steel @ 120");
    app.log("");
    app.log("  PRODUCTION");
    app.log("  recipes (r)               List production recipes");
    app.log("  produce <recipe> [qty]    Start production");
    app.log("    Example: produce smelt_steel 5");
    app.log("");
    app.log("  INFORMATION");
    app.log("  status (st)               Company overview");
    app.log("  inventory (inv, i)        View company inventory");
    app.log("  balance (bal)             View player balance");
    app.log("");
    app.log("  INTERFACE");
    app.log("  Tab / Shift+Tab           Switch panels");
    app.log("  Up/Down                   Scroll log / history");
    app.log("  Esc                       Normal mode");
    app.log("  : or /                    Enter command mode");
    app.log("  clear                     Clear log");
    app.log("  Ctrl+C or q               Quit");
    app.log("===================================================");
}

fn require_company(app: &mut App) -> Option<(Uuid, Uuid)> {
    let pid = match app.player_id { Some(id) => id, None => { app.log_error("Login first."); return None; } };
    let cid = match app.active_company_id { Some(id) => id, None => { app.log_error("No company selected. Use 'create company <name>' or 'select <id>'."); return None; } };
    Some((pid, cid))
}

async fn handle_result(app: &mut App, result: Result<serde_json::Value, anyhow::Error>) {
    match result {
        Ok(v) => {
            let success = v["success"].as_bool().unwrap_or(false);
            let msg = v["message"].as_str().unwrap_or("-");
            if success { app.log_success(msg); } else { app.log_error(msg); }
            app.refresh_company().await;
        }
        Err(e) => app.log_error(&format!("Request failed: {e}")),
    }
}

fn tokenize(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    for ch in input.chars() {
        match ch {
            '"' | '\'' => { in_quotes = !in_quotes; }
            ' ' if !in_quotes => { if !current.is_empty() { tokens.push(current.clone()); current.clear(); } }
            _ => current.push(ch),
        }
    }
    if !current.is_empty() { tokens.push(current); }
    tokens
}

fn truncate_decimal(s: &str) -> String {
    if let Some(dot) = s.find('.') { let end = (dot + 3).min(s.len()); s[..end].to_string() } else { s.to_string() }
}
