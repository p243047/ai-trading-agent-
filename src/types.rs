use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

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

/// Trade signal output with full execution parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeSignal {
    pub symbol: String,
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
