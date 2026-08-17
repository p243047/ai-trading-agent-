use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// Market data snapshot from exchange APIs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketMetrics {
    pub symbol: String,
    pub current_price: f64,
    pub funding_rate: f64,
    pub open_interest: f64,
    pub oi_change_24h_pct: f64,
    pub price_change_24h_pct: f64,
    pub bid_volume_1pct: f64,
    pub ask_volume_1pct: f64,
    pub blackrock_flow_usd: f64,
    pub timestamp: DateTime<Utc>,
}

impl MarketMetrics {
    pub fn liquidity_imbalance(&self) -> f64 {
        let total = self.bid_volume_1pct + self.ask_volume_1pct;
        if total == 0.0 {
            0.0
        } else {
            (self.bid_volume_1pct - self.ask_volume_1pct) / total
        }
    }
}

/// Trade signal output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeSignal {
    pub symbol: String,
    pub action: String, // "LONG", "SHORT", "HOLD"
    pub market_type: String, // "SPOT", "FUTURES"
    pub win_probability: f64,
    pub current_price: f64,
    pub entry_price: f64,
    pub target_price: f64,
    pub stop_loss: f64,
    pub recommended_position_pct: f64,
    pub kelly_fraction: f64,
    pub timestamp: DateTime<Utc>,
}

/// AI verification result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiVerification {
    pub approved: bool,
    pub confidence_adjustment: f64,
    pub reason: String,
}

/// Paper trade record for logging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaperTrade {
    pub trade_id: String,
    pub symbol: String,
    pub action: String,
    pub market_type: String,
    pub entry_price: f64,
    pub entry_time: DateTime<Utc>,
    pub target_price: f64,
    pub stop_loss: f64,
    pub position_size_pct: f64,
    pub status: String, // "OPEN", "CLOSED"
    pub exit_price: Option<f64>,
    pub exit_time: Option<DateTime<Utc>>,
    pub pnl_pct: Option<f64>,
    pub pnl_usd: Option<f64>,
    pub success: Option<bool>,
}

/// Portfolio tracking
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Portfolio {
    pub initial_balance: f64,
    pub current_balance: f64,
    pub total_trades: u32,
    pub winning_trades: u32,
    pub losing_trades: u32,
    pub win_rate: f64,
}

impl Portfolio {
    pub fn new(initial_balance: f64) -> Self {
        Portfolio {
            initial_balance,
            current_balance: initial_balance,
            ..Default::default()
        }
    }

    pub fn update_stats(&mut self, pnl_usd: f64, is_winner: bool) {
        self.current_balance += pnl_usd;
        self.total_trades += 1;
        if is_winner {
            self.winning_trades += 1;
        } else {
            self.losing_trades += 1;
        }
        if self.total_trades > 0 {
            self.win_rate = self.winning_trades as f64 / self.total_trades as f64;
        }
    }
}
