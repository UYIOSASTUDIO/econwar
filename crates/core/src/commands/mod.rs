//! Player command system.
//!
//! Players interact with the game by issuing commands via the terminal-style UI
//! or REST API.  Each command is parsed into a `GameCommand` enum, validated,
//! and processed by the server's command handler.
//!
//! Commands are intentionally designed to feel like CLI operations:
//!   scan_market copper
//!   buy_resource copper 100 @50.00
//!   produce electronics 5
//!   build_factory

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Every action a player can take, expressed as a typed command.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum GameCommand {
    // ── Company Management ──────────────────────────────────────────
    CreateCompany {
        name: String,
    },
    HireWorkers {
        company_id: Uuid,
        count: i32,
    },
    BuildFactory {
        company_id: Uuid,
    },
    ResearchTechnology {
        company_id: Uuid,
    },
    FundCompany {
        company_id: Uuid,
        amount: Decimal,
    },

    // ── Market Operations ───────────────────────────────────────────
    ScanMarket {
        resource_slug: String,
    },
    ScanAllMarkets,
    ViewOrderBook {
        resource_slug: String,
    },
    PriceHistory {
        resource_slug: String,
        /// Number of snapshots to return (default: 50).
        limit: Option<i32>,
    },

    // ── Trading ─────────────────────────────────────────────────────
    BuyResource {
        company_id: Uuid,
        resource_slug: String,
        quantity: Decimal,
        /// Max price per unit (limit order).
        max_price: Decimal,
    },
    SellResource {
        company_id: Uuid,
        resource_slug: String,
        quantity: Decimal,
        /// Min price per unit (limit order).
        min_price: Decimal,
    },
    CancelOrder {
        order_id: Uuid,
    },

    // ── Production ──────────────────────────────────────────────────
    ListRecipes,
    StartProduction {
        company_id: Uuid,
        recipe_slug: String,
        batch_size: i32,
    },
    ViewProduction {
        company_id: Uuid,
    },

    // ── Information ─────────────────────────────────────────────────
    ViewCompany {
        company_id: Uuid,
    },
    ViewInventory {
        company_id: Uuid,
    },
    ViewBalance,
    ListCompanies,

    // ── Chat ────────────────────────────────────────────────────────
    GlobalChat {
        message: String,
    },
}

/// Unified response type for all commands.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResult {
    pub success: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl CommandResult {
    pub fn ok(message: impl Into<String>, data: Option<serde_json::Value>) -> Self {
        Self {
            success: true,
            message: message.into(),
            data,
        }
    }

    pub fn err(message: impl Into<String>) -> Self {
        Self {
            success: false,
            message: message.into(),
            data: None,
        }
    }
}
