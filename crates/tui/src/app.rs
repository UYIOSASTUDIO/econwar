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
