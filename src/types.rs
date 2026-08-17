use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Represents the current market metrics for a trading symbol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketMetrics {
    pub symbol: String,
    pub current_price: f64,
    pub open_interest: f64,
    pub open_interest_24h_change: f64,
    pub funding_rate: f64,
    pub funding_rate_avg_7d: f64,
    pub funding_rate_std_7d: f64,
    pub bid_volume_1pct: f64,
    pub ask_volume_1pct: f64,
    pub blackrock_flow_usd: f64,
    pub blackrock_flow_mean_30d: f64,
    pub blackrock_flow_std_30d: f64,
    pub timestamp: DateTime<Utc>,
}

impl MarketMetrics {
    pub fn new(symbol: &str) -> Self {
        MarketMetrics {
            symbol: symbol.to_string(),
            current_price: 0.0,
            open_interest: 0.0,
            open_interest_24h_change: 0.0,
            funding_rate: 0.0,
            funding_rate_avg_7d: 0.0,
            funding_rate_std_7d: 0.0,
            bid_volume_1pct: 0.0,
            ask_volume_1pct: 0.0,
            blackrock_flow_usd: 0.0,
            blackrock_flow_mean_30d: 0.0,
            blackrock_flow_std_30d: 0.0,
            timestamp: Utc::now(),
        }
    }
}

/// Represents the three market regimes for Markov state transitions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketRegime {
    RangeBound,    // S0: Mean-Reverting
    Bullish,       // S1: Trending Up
    Bearish,       // S2: Trending Down
}

/// AI verification response from local LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIVerification {
    pub approved: bool,
    pub confidence_adjustment: f64,
    pub reason: String,
}

impl Default for AIVerification {
    fn default() -> Self {
        AIVerification {
            approved: true,
            confidence_adjustment: 0.0,
            reason: "No AI review performed".to_string(),
        }
    }
}

/// Type of trade: Spot or Futures
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TradeType {
    Spot,
    Futures,
}

/// Trade direction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Long,
    Short,
}

/// Paper trade record with entry and exit details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaperTrade {
    pub trade_id: String,
    pub symbol: String,
    pub coin_name: String,
    pub trade_type: TradeType,
    pub direction: Direction,
    pub action: String, // "LONG" or "SHORT"
    pub win_probability: f64,
    pub entry_price: f64,
    pub target_price: f64,
    pub stop_loss: f64,
    pub position_size_pct: f64,
    pub kelly_fraction: f64,
    pub entry_time: DateTime<Utc>,
    pub exit_time: Option<DateTime<Utc>>,
    pub exit_price: Option<f64>,
    pub pnl_pct: Option<f64>,
    pub pnl_usd: Option<f64>,
    pub status: String, // "OPEN", "CLOSED", "STOPPED", "TARGET_HIT"
    pub success: Option<bool>,
}

impl PaperTrade {
    pub fn new(
        symbol: &str,
        coin_name: &str,
        trade_type: TradeType,
        direction: Direction,
        action: &str,
        win_probability: f64,
        entry_price: f64,
        target_price: f64,
        stop_loss: f64,
        position_size_pct: f64,
        kelly_fraction: f64,
    ) -> Self {
        PaperTrade {
            trade_id: Uuid::new_v4().to_string(),
            symbol: symbol.to_string(),
            coin_name: coin_name.to_string(),
            trade_type,
            direction,
            action: action.to_string(),
            win_probability,
            entry_price,
            target_price,
            stop_loss,
            position_size_pct,
            kelly_fraction,
            entry_time: Utc::now(),
            exit_time: None,
            exit_price: None,
            pnl_pct: None,
            pnl_usd: None,
            status: "OPEN".to_string(),
            success: None,
        }
    }

    pub fn close_trade(&mut self, exit_price: f64, status: &str) {
        self.exit_time = Some(Utc::now());
        self.exit_price = Some(exit_price);
        self.status = status.to_string();

        // Calculate PnL based on direction
        let pnl = match self.direction {
            Direction::Long => (exit_price - self.entry_price) / self.entry_price * 100.0,
            Direction::Short => (self.entry_price - exit_price) / self.entry_price * 100.0,
        };
        
        self.pnl_pct = Some(pnl);
        self.pnl_usd = Some(pnl * self.position_size_pct); // Simplified: assumes 1 unit
        
        // Determine success based on whether we hit target before stop
        self.success = match status {
            "TARGET_HIT" => Some(true),
            "STOPPED" => Some(false),
            _ => {
                if pnl > 0.0 {
                    Some(true)
                } else {
                    Some(false)
                }
            }
        };
    }
}

/// Trade signal output with full execution parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeSignal {
    pub symbol: String,
    pub coin_name: String,
    pub trade_type: TradeType,
    pub action: String, // "LONG", "SHORT", or "HOLD"
    pub win_probability: f64,
    pub current_price: f64,
    pub target_price: f64,
    pub stop_loss: f64,
    pub recommended_position_pct: f64,
    pub kelly_fraction: f64,
    pub timestamp: DateTime<Utc>,
}

impl TradeSignal {
    pub fn hold(symbol: &str) -> Self {
        TradeSignal {
            symbol: symbol.to_string(),
            coin_name: symbol.replace("USDT", "").to_string(),
            trade_type: TradeType::Futures,
            action: "HOLD".to_string(),
            win_probability: 0.5,
            current_price: 0.0,
            target_price: 0.0,
            stop_loss: 0.0,
            recommended_position_pct: 0.0,
            kelly_fraction: 0.0,
            timestamp: Utc::now(),
        }
    }
}
