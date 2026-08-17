//! Kelly Criterion & Position Sizing Engine
//! 
//! Implements fractional Kelly formula for optimal position sizing

use crate::types::{TradeSignal, MarketMetrics, TradeType};
use chrono::Utc;

/// Safety fraction to prevent drawdown spikes (25% of full Kelly)
const KELLY_FRACTION: f64 = 0.25;

/// Maximum capital risk per trade (1.5% of account balance)
const MAX_RISK_PCT: f64 = 0.015;

/// Minimum probability threshold for taking a trade
const MIN_PROBABILITY: f64 = 0.45;

/// Default risk-to-reward ratio targets
const DEFAULT_RISK_REWARD_RATIO: f64 = 2.0;

/// Stop loss percentage (default 2% from entry)
const DEFAULT_STOP_LOSS_PCT: f64 = 0.02;

/// Take profit percentage (default 4% from entry for 2:1 R:R)
const DEFAULT_TAKE_PROFIT_PCT: f64 = 0.04;

/// Generate trade signal with Kelly-based position sizing
/// 
/// Uses Fractional Kelly Formula:
/// f* = φ * ((p * b - (1 - p)) / b)
/// 
/// Where:
/// - p = P_win (calculated Bayesian probability)
/// - b = Risk-to-Reward Ratio (TP / SL distance)
/// - φ = Safety fraction (0.25)
pub fn generate_trade_signal(
    symbol: &str,
    win_probability: f64,
    current_price: f64,
) -> TradeSignal {
    // Check minimum probability threshold
    if win_probability < MIN_PROBABILITY {
        return TradeSignal::hold(symbol);
    }
    
    // Calculate risk-to-reward ratio
    let b = DEFAULT_RISK_REWARD_RATIO;
    
    // Calculate full Kelly fraction
    let kelly_full = (win_probability * b - (1.0 - win_probability)) / b;
    
    // Apply fractional Kelly (safety adjustment)
    let kelly_fraction = KELLY_FRACTION * kelly_full.max(0.0);
    
    // Apply maximum risk cap
    let position_pct = kelly_fraction.min(MAX_RISK_PCT);
    
    // Determine direction based on probability
    let action = if win_probability >= 0.55 {
        "LONG"
    } else if win_probability <= 0.45 {
        "SHORT"
    } else {
        return TradeSignal::hold(symbol);
    };
    
    // Calculate stop loss and take profit levels
    let (stop_loss, target_price) = calculate_price_levels(current_price, action);
    
    TradeSignal {
        symbol: symbol.to_string(),
        coin_name: symbol.replace("USDT", "").to_string(),
        trade_type: TradeType::Spot,  // Default, will be overridden in main.rs
        action: action.to_string(),
        win_probability,
        current_price,
        target_price,
        stop_loss,
        recommended_position_pct: position_pct,
        kelly_fraction,
        timestamp: Utc::now(),
    }
}

/// Calculate stop loss and take profit price levels
fn calculate_price_levels(current_price: f64, action: &str) -> (f64, f64) {
    match action {
        "LONG" => {
            let stop_loss = current_price * (1.0 - DEFAULT_STOP_LOSS_PCT);
            let target_price = current_price * (1.0 + DEFAULT_TAKE_PROFIT_PCT);
            (stop_loss, target_price)
        }
        "SHORT" => {
            let stop_loss = current_price * (1.0 + DEFAULT_STOP_LOSS_PCT);
            let target_price = current_price * (1.0 - DEFAULT_TAKE_PROFIT_PCT);
            (stop_loss, target_price)
        }
        _ => (0.0, 0.0),
    }
}

/// Adjust position size based on volatility regime
/// During high volatility, cut position sizes in half
pub fn adjust_for_volatility(position_pct: f64, current_volatility: f64, baseline_volatility: f64) -> f64 {
    const VOLATILITY_SPIKE_THRESHOLD: f64 = 3.0;
    
    let vol_ratio = current_volatility / baseline_volatility.max(0.0001);
    
    if vol_ratio > VOLATILITY_SPIKE_THRESHOLD {
        // High volatility regime - halve position size
        position_pct * 0.5
    } else {
        position_pct
    }
}

/// Calculate optimal stop loss distance using ATR (Average True Range)
pub fn calculate_atr_stop_loss(entry_price: f64, atr: f64, multiplier: f64) -> f64 {
    entry_price - (atr * multiplier)
}

/// Calculate optimal take profit using risk-reward ratio
pub fn calculate_take_profit(entry_price: f64, stop_loss: f64, risk_reward_ratio: f64) -> f64 {
    let risk_distance = (entry_price - stop_loss).abs();
    if entry_price > stop_loss {
        // Long position
        entry_price + (risk_distance * risk_reward_ratio)
    } else {
        // Short position
        entry_price - (risk_distance * risk_reward_ratio)
    }
}

/// Dynamic volatility estimation using exponential moving average
pub fn estimate_volatility(prices: &[f64], span: usize) -> f64 {
    if prices.len() < 2 {
        return 0.0;
    }
    
    // Calculate log returns
    let returns: Vec<f64> = prices.windows(2)
        .map(|w| (w[1] / w[0]).ln())
        .collect();
    
    // Calculate EMA of squared returns (variance proxy)
    let mut ema_variance = 0.0;
    let alpha = 2.0 / (span as f64 + 1.0);
    
    for ret in returns.iter().take(span) {
        ema_variance = alpha * (ret * ret) + (1.0 - alpha) * ema_variance;
    }
    
    // Volatility is sqrt of variance
    ema_variance.sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kelly_calculation() {
        // Test with 60% win probability and 2:1 reward ratio
        let signal = generate_trade_signal("BTCUSDT", 0.60, 95000.0);
        
        assert_eq!(signal.action, "LONG");
        assert!(signal.win_probability >= 0.55);
        assert!(signal.recommended_position_pct <= MAX_RISK_PCT);
        assert!(signal.stop_loss < signal.current_price);
        assert!(signal.target_price > signal.current_price);
    }

    #[test]
    fn test_hold_signal() {
        let signal = generate_trade_signal("BTCUSDT", 0.50, 95000.0);
        assert_eq!(signal.action, "HOLD");
    }

    #[test]
    fn test_volatility_adjustment() {
        let base_position = 0.01;
        // High volatility: 0.15 / 0.05 = 3.0, which equals threshold so no adjustment
        let adjusted_normal = adjust_for_volatility(base_position, 0.14, 0.05);
        assert!((adjusted_normal - base_position).abs() < 0.0001);
        
        // Very high volatility: 0.15 / 0.05 = 3.0+, triggers halving
        let adjusted_high = adjust_for_volatility(base_position, 0.16, 0.05);
        assert!((adjusted_high - base_position * 0.5).abs() < 0.0001);
    }

    #[test]
    fn test_price_levels_long() {
        let (sl, tp) = calculate_price_levels(100.0, "LONG");
        assert!((sl - 98.0).abs() < 0.01); // 2% below
        assert!((tp - 104.0).abs() < 0.01); // 4% above
    }

    #[test]
    fn test_price_levels_short() {
        let (sl, tp) = calculate_price_levels(100.0, "SHORT");
        assert!((sl - 102.0).abs() < 0.01); // 2% above
        assert!((tp - 96.0).abs() < 0.01); // 4% below
    }
}
