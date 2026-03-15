#!/bin/bash
# Run this from the econwar project root to create the TUI crate.
# Usage: bash scripts/create_tui.sh

set -e

echo "Creating TUI crate structure..."
mkdir -p crates/tui/src

echo "Writing crates/tui/Cargo.toml..."
cat > crates/tui/Cargo.toml << 'TOMLEOF'
[package]
name = "econwar-tui"
version.workspace = true
edition.workspace = true

[[bin]]
name = "econwar"
path = "src/main.rs"

[dependencies]
ratatui = "0.29"
crossterm = "0.28"
reqwest = { version = "0.12", features = ["json"] }
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
uuid = { workspace = true }
chrono = { workspace = true }
anyhow = { workspace = true }
TOMLEOF

echo "Writing crates/tui/src/main.rs..."
cat > crates/tui/src/main.rs << 'RSEOF'
//! EconWar TUI Client

mod app;
mod api;
mod ui;
mod commands;

use std::io;
use std::time::Duration;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;

use app::{App, InputMode};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let server_url = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "http://localhost:8080".to_string());

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(server_url);
    let result = run_app(&mut terminal, &mut app).await;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(e) = result {
        eprintln!("Error: {e:#}");
    }

    Ok(())
}

async fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> anyhow::Result<()> {
    app.log("Welcome to EconWar! Type 'help' for commands, or 'login <user> <pass>' to start.");
    app.log("Press Tab to switch panels, Esc to unfocus, Ctrl+C to quit.");

    loop {
        terminal.draw(|f| ui::draw(f, app))?;

        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c')
                {
                    return Ok(());
                }

                match app.input_mode {
                    InputMode::Normal => match key.code {
                        KeyCode::Char(':') | KeyCode::Char('/') => {
                            app.input_mode = InputMode::Command;
                        }
                        KeyCode::Tab => app.next_panel(),
                        KeyCode::BackTab => app.prev_panel(),
                        KeyCode::Char('q') => return Ok(()),
                        KeyCode::Char('?') => {
                            commands::handle_command(app, "help").await;
                        }
                        KeyCode::Up => app.scroll_up(),
                        KeyCode::Down => app.scroll_down(),
                        _ => {}
                    },
                    InputMode::Command => match key.code {
                        KeyCode::Enter => {
                            let input = app.input.drain(..).collect::<String>();
                            if !input.is_empty() {
                                app.command_history.push(input.clone());
                                app.history_index = app.command_history.len();
                                commands::handle_command(app, &input).await;
                            }
                        }
                        KeyCode::Char(c) => {
                            app.input.push(c);
                        }
                        KeyCode::Backspace => {
                            app.input.pop();
                        }
                        KeyCode::Esc => {
                            app.input.clear();
                            app.input_mode = InputMode::Normal;
                        }
                        KeyCode::Up => {
                            if app.history_index > 0 {
                                app.history_index -= 1;
                                app.input = app.command_history[app.history_index].chars().collect();
                            }
                        }
                        KeyCode::Down => {
                            if app.history_index < app.command_history.len() {
                                app.history_index += 1;
                                if app.history_index < app.command_history.len() {
                                    app.input =
                                        app.command_history[app.history_index].chars().collect();
                                } else {
                                    app.input.clear();
                                }
                            }
                        }
                        _ => {}
                    },
                }
            }
        }

        app.tick_counter += 1;
        if app.tick_counter % 20 == 0 && app.token.is_some() {
            app.refresh_markets().await;
        }
    }
}
RSEOF

echo "Writing crates/tui/src/app.rs..."
cat > crates/tui/src/app.rs << 'RSEOF'
//! Application state for the TUI.

use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    Log,
    Markets,
    Company,
}

impl Panel {
    pub fn next(self) -> Self {
        match self {
            Panel::Log => Panel::Markets,
            Panel::Markets => Panel::Company,
            Panel::Company => Panel::Log,
        }
    }
    pub fn prev(self) -> Self {
        match self {
            Panel::Log => Panel::Company,
            Panel::Markets => Panel::Log,
            Panel::Company => Panel::Markets,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Command,
}

#[derive(Debug, Clone)]
pub struct MarketRow {
    pub slug: String,
    pub name: String,
    pub price: String,
    pub ema: String,
    pub supply: String,
    pub demand: String,
}

#[derive(Debug, Clone)]
pub struct CompanyInfo {
    pub id: String,
    pub name: String,
    pub treasury: String,
    pub workers: String,
    pub capacity: String,
    pub factories: String,
    pub tech_level: String,
}

#[derive(Debug, Clone)]
pub struct InventoryItem {
    pub resource: String,
    pub quantity: String,
}

pub struct App {
    pub server_url: String,
    pub input: Vec<char>,
    pub input_mode: InputMode,
    pub active_panel: Panel,
    pub token: Option<String>,
    pub player_id: Option<Uuid>,
    pub username: Option<String>,
    pub active_company_id: Option<Uuid>,
    pub active_company: Option<CompanyInfo>,
    pub inventory: Vec<InventoryItem>,
    pub markets: Vec<MarketRow>,
    pub log_messages: Vec<String>,
    pub log_scroll: usize,
    pub command_history: Vec<String>,
    pub history_index: usize,
    pub tick_counter: u64,
}

impl App {
    pub fn new(server_url: String) -> Self {
        Self {
            server_url,
            input: Vec::new(),
            input_mode: InputMode::Command,
            active_panel: Panel::Log,
            token: None,
            player_id: None,
            username: None,
            active_company_id: None,
            active_company: None,
            inventory: Vec::new(),
            markets: Vec::new(),
            log_messages: Vec::new(),
            log_scroll: 0,
            command_history: Vec::new(),
            history_index: 0,
            tick_counter: 0,
        }
    }

    pub fn log(&mut self, msg: &str) {
        let timestamp = chrono::Local::now().format("%H:%M:%S");
        self.log_messages.push(format!("[{timestamp}] {msg}"));
        if self.log_messages.len() > 1 {
            self.log_scroll = self.log_messages.len().saturating_sub(1);
        }
    }

    pub fn log_success(&mut self, msg: &str) {
        self.log(&format!("OK: {msg}"));
    }

    pub fn log_error(&mut self, msg: &str) {
        self.log(&format!("ERR: {msg}"));
    }

    pub fn next_panel(&mut self) {
        self.active_panel = self.active_panel.next();
    }

    pub fn prev_panel(&mut self) {
        self.active_panel = self.active_panel.prev();
    }

    pub fn scroll_up(&mut self) {
        self.log_scroll = self.log_scroll.saturating_sub(1);
    }

    pub fn scroll_down(&mut self) {
        if self.log_scroll < self.log_messages.len().saturating_sub(1) {
            self.log_scroll += 1;
        }
    }

    pub async fn refresh_markets(&mut self) {
        match crate::api::get_markets(&self.server_url).await {
            Ok(rows) => self.markets = rows,
            Err(_) => {}
        }
    }

    pub async fn refresh_company(&mut self) {
        if let Some(cid) = self.active_company_id {
            if let Ok(info) = crate::api::get_company(&self.server_url, cid).await {
                self.active_company = Some(info);
            }
            if let Ok(inv) = crate::api::get_inventory(&self.server_url, cid).await {
                self.inventory = inv;
            }
        }
    }
}
RSEOF

echo "Writing crates/tui/src/api.rs..."
cat > crates/tui/src/api.rs << 'RSEOF'
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
RSEOF

echo "Writing crates/tui/src/ui.rs..."
cat > crates/tui/src/ui.rs << 'RSEOF'
//! TUI rendering with Ratatui.

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, Paragraph, Row, Table},
};

use crate::app::{App, InputMode, Panel};

pub fn draw(f: &mut Frame, app: &App) {
    let size = f.area();
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(size);

    draw_header(f, app, main_chunks[0]);
    draw_body(f, app, main_chunks[1]);
    draw_command_bar(f, app, main_chunks[2]);
}

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let player = app.username.as_deref().unwrap_or("not logged in");
    let company = app.active_company.as_ref().map(|c| c.name.as_str()).unwrap_or("none");
    let header_text = format!(
        " ECONWAR v0.1  |  Player: {}  |  Company: {}  |  Type 'help' for commands",
        player, company
    );
    let header = Paragraph::new(header_text)
        .style(Style::default().fg(Color::Black).bg(Color::Cyan))
        .block(Block::default());
    f.render_widget(header, area);
}

fn draw_body(f: &mut Frame, app: &App, area: Rect) {
    let body_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    draw_log_panel(f, app, body_chunks[0]);

    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(body_chunks[1]);

    draw_market_panel(f, app, right_chunks[0]);
    draw_company_panel(f, app, right_chunks[1]);
}

fn draw_log_panel(f: &mut Frame, app: &App, area: Rect) {
    let is_active = app.active_panel == Panel::Log;
    let border_style = if is_active {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let block = Block::default()
        .title(" Log ")
        .borders(Borders::ALL)
        .border_style(border_style);
    let inner = block.inner(area);
    f.render_widget(block, area);

    if app.log_messages.is_empty() { return; }

    let visible_height = inner.height as usize;
    let total = app.log_messages.len();
    let start = if total <= visible_height { 0 } else {
        app.log_scroll.min(total.saturating_sub(visible_height))
    };
    let end = (start + visible_height).min(total);

    let items: Vec<ListItem> = app.log_messages[start..end]
        .iter()
        .map(|msg| {
            let style = if msg.contains("OK:") {
                Style::default().fg(Color::Green)
            } else if msg.contains("ERR:") {
                Style::default().fg(Color::Red)
            } else if msg.contains("===") || msg.contains("---") {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(Line::from(msg.as_str())).style(style)
        })
        .collect();

    let list = List::new(items);
    f.render_widget(list, inner);
}

fn draw_market_panel(f: &mut Frame, app: &App, area: Rect) {
    let is_active = app.active_panel == Panel::Markets;
    let border_style = if is_active {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let block = Block::default()
        .title(" Markets (live) ")
        .borders(Borders::ALL)
        .border_style(border_style);

    if app.markets.is_empty() {
        let msg = Paragraph::new(" No data. Login to see markets.")
            .style(Style::default().fg(Color::DarkGray))
            .block(block);
        f.render_widget(msg, area);
        return;
    }

    let header = Row::new(vec!["Resource", "Price", "Supply", "Demand"])
        .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
        .bottom_margin(0);

    let rows: Vec<Row> = app.markets.iter().map(|m| {
        Row::new(vec![
            m.slug.clone(),
            truncate(&m.price),
            truncate(&m.supply),
            truncate(&m.demand),
        ]).style(Style::default().fg(Color::White))
    }).collect();

    let widths = [
        Constraint::Length(14),
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(10),
    ];

    let table = Table::new(rows, widths).header(header).block(block);
    f.render_widget(table, area);
}

fn draw_company_panel(f: &mut Frame, app: &App, area: Rect) {
    let is_active = app.active_panel == Panel::Company;
    let border_style = if is_active {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let block = Block::default()
        .title(" Company ")
        .borders(Borders::ALL)
        .border_style(border_style);
    let inner = block.inner(area);
    f.render_widget(block, area);

    match &app.active_company {
        Some(c) => {
            let text = vec![
                Line::from(vec![
                    Span::styled(" Name:      ", Style::default().fg(Color::DarkGray)),
                    Span::styled(&c.name, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                ]),
                Line::from(vec![
                    Span::styled(" Treasury:  ", Style::default().fg(Color::DarkGray)),
                    Span::styled(format!("{} credits", c.treasury), Style::default().fg(Color::Green)),
                ]),
                Line::from(vec![
                    Span::styled(" Workers:   ", Style::default().fg(Color::DarkGray)),
                    Span::styled(format!("{} / {}", c.workers, c.capacity), Style::default().fg(Color::White)),
                ]),
                Line::from(vec![
                    Span::styled(" Factories: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(&c.factories, Style::default().fg(Color::White)),
                ]),
                Line::from(vec![
                    Span::styled(" Tech Level:", Style::default().fg(Color::DarkGray)),
                    Span::styled(format!(" {}", c.tech_level), Style::default().fg(Color::Magenta)),
                ]),
            ];
            let paragraph = Paragraph::new(text);
            f.render_widget(paragraph, inner);
        }
        None => {
            let msg = Paragraph::new(" No company selected.")
                .style(Style::default().fg(Color::DarkGray));
            f.render_widget(msg, inner);
        }
    }
}

fn draw_command_bar(f: &mut Frame, app: &App, area: Rect) {
    let (prompt, style) = match app.input_mode {
        InputMode::Command => (" > ", Style::default().fg(Color::Cyan)),
        InputMode::Normal => (" NORMAL (press : or / to type) ", Style::default().fg(Color::DarkGray)),
    };
    let input_str: String = app.input.iter().collect();
    let display = format!("{prompt}{input_str}");
    let input_widget = Paragraph::new(display)
        .style(style)
        .block(Block::default().borders(Borders::ALL).border_style(
            match app.input_mode {
                InputMode::Command => Style::default().fg(Color::Cyan),
                InputMode::Normal => Style::default().fg(Color::DarkGray),
            },
        ));
    f.render_widget(input_widget, area);

    if app.input_mode == InputMode::Command {
        let cursor_x = area.x + prompt.len() as u16 + input_str.len() as u16 + 1;
        let cursor_y = area.y + 1;
        f.set_cursor_position(Position::new(cursor_x, cursor_y));
    }
}

fn truncate(s: &str) -> String {
    if let Some(dot) = s.find('.') {
        let end = (dot + 3).min(s.len());
        s[..end].to_string()
    } else {
        s.to_string()
    }
}
RSEOF

echo "Writing crates/tui/src/commands.rs..."
cat > crates/tui/src/commands.rs << 'RSEOF'
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
    if let Some(name) = &app.username {
        app.log(&format!("Player: {name}"));
    } else { app.log("Not logged in."); return; }
    cmd_balance(app).await;
    if app.active_company_id.is_some() {
        app.refresh_company().await;
        if let Some(c) = &app.active_company {
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
RSEOF

echo ""
echo "TUI crate created successfully!"
echo "Now run: cargo build --release --bin econwar"
echo "Then:    cargo run --release --bin econwar"