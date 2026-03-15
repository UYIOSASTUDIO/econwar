#!/usr/bin/env bash
# Patch: adds PageUp/PageDown scrolling to the TUI log panel.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

echo "Patching TUI scrolling..."

# ── app.rs ──────────────────────────────────────────────────────
cat > "$PROJECT_DIR/crates/tui/src/app.rs" << 'APPEOF'
//! Application state for the TUI.

use uuid::Uuid;

/// Which panel is currently focused.
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

/// Market data row for display.
#[derive(Debug, Clone)]
pub struct MarketRow {
    pub slug: String,
    pub name: String,
    pub price: String,
    pub ema: String,
    pub supply: String,
    pub demand: String,
}

/// Company summary for display.
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

/// Inventory item for display.
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

    // Auth state.
    pub token: Option<String>,
    pub player_id: Option<Uuid>,
    pub username: Option<String>,

    // Active company.
    pub active_company_id: Option<Uuid>,
    pub active_company: Option<CompanyInfo>,
    pub inventory: Vec<InventoryItem>,

    // Market data.
    pub markets: Vec<MarketRow>,

    // Log / output panel.
    pub log_messages: Vec<String>,
    pub log_scroll: usize,

    // Command history.
    pub command_history: Vec<String>,
    pub history_index: usize,

    // Tick counter for periodic refresh.
    pub tick_counter: u64,
}

impl App {
    pub fn new(server_url: String) -> Self {
        Self {
            server_url,
            input: Vec::new(),
            input_mode: InputMode::Command, // Start in command mode.
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

    /// Whether the user is at the bottom of the log (auto-scroll enabled).
    fn is_at_bottom(&self) -> bool {
        // Consider "at bottom" if within 3 lines of the end.
        self.log_scroll + 3 >= self.log_messages.len()
    }

    pub fn log(&mut self, msg: &str) {
        let was_at_bottom = self.is_at_bottom();
        let timestamp = chrono::Local::now().format("%H:%M:%S");
        self.log_messages.push(format!("[{timestamp}] {msg}"));
        // Only auto-scroll if user was already at the bottom.
        // If they scrolled up to read, don't yank them away.
        if was_at_bottom {
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

    /// Scroll up by a full page (10 lines).
    pub fn scroll_up_page(&mut self) {
        self.log_scroll = self.log_scroll.saturating_sub(10);
    }

    /// Scroll down by a full page (10 lines).
    pub fn scroll_down_page(&mut self) {
        self.log_scroll = (self.log_scroll + 10).min(
            self.log_messages.len().saturating_sub(1),
        );
    }

    /// Jump to the bottom of the log.
    pub fn scroll_to_bottom(&mut self) {
        self.log_scroll = self.log_messages.len().saturating_sub(1);
    }

    /// Refresh market data from the server.
    pub async fn refresh_markets(&mut self) {
        match crate::api::get_markets(&self.server_url).await {
            Ok(rows) => self.markets = rows,
            Err(_) => {} // Silently ignore refresh errors.
        }
    }

    /// Refresh the active company and its inventory.
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
APPEOF

# ── main.rs ─────────────────────────────────────────────────────
cat > "$PROJECT_DIR/crates/tui/src/main.rs" << 'MAINEOF'
//! EconWar TUI Client
//!
//! A terminal-based dashboard for playing EconWar.
//! Connect to a running server and manage your economic empire.

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
    // Parse server URL from args or default.
    let server_url = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "http://localhost:8080".to_string());

    // Setup terminal.
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run the app.
    let mut app = App::new(server_url);
    let result = run_app(&mut terminal, &mut app).await;

    // Restore terminal.
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
    // Show welcome message.
    app.log("Welcome to EconWar! Type 'help' for commands, or 'login <user> <pass>' to start.");
    app.log("Press Tab to switch panels, Esc to unfocus, Ctrl+C to quit.");
    app.log("Scroll: PageUp/PageDown (any mode), Up/Down/Home/End (normal mode).");

    loop {
        terminal.draw(|f| ui::draw(f, app))?;

        // Poll for events with a timeout so we can do periodic refreshes.
        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                // Global quit: Ctrl+C always exits.
                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c')
                {
                    return Ok(());
                }

                // PageUp/PageDown scroll the log in ANY mode.
                match key.code {
                    KeyCode::PageUp => { app.scroll_up_page(); continue; }
                    KeyCode::PageDown => { app.scroll_down_page(); continue; }
                    _ => {}
                }

                match app.input_mode {
                    InputMode::Normal => match key.code {
                        KeyCode::Char(':') | KeyCode::Char('/') => {
                            app.input_mode = InputMode::Command;
                        }
                        KeyCode::Tab => {
                            app.next_panel();
                        }
                        KeyCode::BackTab => {
                            app.prev_panel();
                        }
                        KeyCode::Char('q') => return Ok(()),
                        KeyCode::Char('?') => {
                            commands::handle_command(app, "help").await;
                        }
                        KeyCode::Up => app.scroll_up(),
                        KeyCode::Down => app.scroll_down(),
                        KeyCode::Home => { app.log_scroll = 0; }
                        KeyCode::End => { app.scroll_to_bottom(); }
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

        // Periodic market refresh (every ~5 seconds).
        app.tick_counter += 1;
        if app.tick_counter % 20 == 0 && app.token.is_some() {
            app.refresh_markets().await;
        }
    }
}
MAINEOF

echo "Done! Rebuild with:  cargo build --release --bin econwar"