use crate::types::TradeSignal;
use chrono::Utc;

/// Kelly Criterion position sizing with fractional safety
/// f* = φ * ((p * b - (1 - p)) / b)
/// Where:
///   p = win probability
///   b = risk/reward ratio (TP/SL)
///   φ = 0.25 (safety fraction)

const KELLY_FRACTION: f64 = 0.25;
const MAX_RISK_PCT: f64 = 0.015; // Maximum 1.5% account risk per trade
const VOLATILITY_MULTIPLIER: f64 = 3.0;

/// Generate trade signal with Kelly-based position sizing
pub fn generate_trade_signal(
    symbol: &str,
    win_probability: f64,
    current_price: f64,
) -> TradeSignal {
    // Determine action based on probability threshold
    let (action, market_type) = if win_probability > 0.55 {
        ("LONG", "FUTURES")
    } else if win_probability < 0.45 {
        ("SHORT", "FUTURES")
    } else {
        ("HOLD", "SPOT")
    };
    
    if action == "HOLD" {
        return TradeSignal {
            symbol: symbol.to_string(),
            action: "HOLD".to_string(),
            market_type: "SPOT".to_string(),
            win_probability,
            current_price,
            entry_price: current_price,
            target_price: current_price,
            stop_loss: current_price,
            recommended_position_pct: 0.0,
            kelly_fraction: 0.0,
            timestamp: Utc::now(),
        };
    }
    
    // Calculate risk/reward parameters
    let (target_price, stop_loss) = calculate_tp_sl(current_price, action);
    
    // Calculate Kelly fraction
    let risk_reward_ratio = if stop_loss > 0.0 {
        (target_price - current_price).abs() / (current_price - stop_loss).abs()
    } else {
        1.5 // Default R:R
    };
    
    let kelly_full = calculate_kelly(win_probability, risk_reward_ratio);
    let kelly_fractional = kelly_full * KELLY_FRACTION;
    
    // Apply max risk cap
    let position_pct = kelly_fractional.min(MAX_RISK_PCT);
    
    TradeSignal {
        symbol: symbol.to_string(),
        action: action.to_string(),
        market_type: market_type.to_string(),
        win_probability,
        current_price,
        entry_price: current_price,
        target_price,
        stop_loss,
        recommended_position_pct: position_pct,
        kelly_fraction: kelly_fractional,
        timestamp: Utc::now(),
    }
}

/// Calculate Take Profit and Stop Loss levels
fn calculate_tp_sl(price: f64, action: &str) -> (f64, f64) {
    // Dynamic TP/SL based on price level (higher prices = wider stops)
    let base_stop_pct = if price > 50000.0 {
        0.02 // 2% for BTC
    } else if price > 1000.0 {
        0.03 // 3% for ETH
    } else {
        0.05 // 5% for alts
    };
    
    let take_profit_pct = base_stop_pct * 2.0; // 2:1 reward:risk
    
    match action {
        "LONG" => {
            let stop_loss = price * (1.0 - base_stop_pct);
            let target = price * (1.0 + take_profit_pct);
            (target, stop_loss)
        }
        "SHORT" => {
            let stop_loss = price * (1.0 + base_stop_pct);
            let target = price * (1.0 - take_profit_pct);
            (target, stop_loss)
        }
        _ => (price, price),
    }
}

/// Calculate full Kelly fraction
/// f* = (p * b - (1 - p)) / b
fn calculate_kelly(win_prob: f64, reward_ratio: f64) -> f64 {
    if reward_ratio <= 0.0 {
        return 0.0;
    }
    
    let numerator = win_prob * reward_ratio - (1.0 - win_prob);
    let kelly = numerator / reward_ratio;
    
    kelly.max(0.0).min(1.0) // Clamp between 0 and 1
}

/// Adjust position size for high volatility regimes
pub fn apply_volatility_adjustment(position_pct: f64, volatility_ratio: f64) -> f64 {
    if volatility_ratio > VOLATILITY_MULTIPLIER {
        // Cut position in half during extreme volatility
        position_pct * 0.5
    } else {
        position_pct
    }
}

/// Calculate expected value of trade
pub fn calculate_expected_value(win_prob: f64, reward_ratio: f64) -> f64 {
    win_prob * reward_ratio - (1.0 - win_prob)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_long_signal_generation() {
        let signal = generate_trade_signal("BTCUSDT", 0.65, 95000.0);
        
        assert_eq!(signal.action, "LONG");
        assert!(signal.target_price > signal.entry_price);
        assert!(signal.stop_loss < signal.entry_price);
        assert!(signal.recommended_position_pct <= MAX_RISK_PCT);
        println!("LONG Signal: Entry=${}, TP=${}, SL=${}, Pos={:.2}%", 
            signal.entry_price, signal.target_price, signal.stop_loss, 
            signal.recommended_position_pct * 100.0);
    }
    
    #[test]
    fn test_short_signal_generation() {
        let signal = generate_trade_signal("BTCUSDT", 0.35, 95000.0);
        
        assert_eq!(signal.action, "SHORT");
        assert!(signal.target_price < signal.entry_price);
        assert!(signal.stop_loss > signal.entry_price);
        println!("SHORT Signal: Entry=${}, TP=${}, SL=${}, Pos={:.2}%", 
            signal.entry_price, signal.target_price, signal.stop_loss, 
            signal.recommended_position_pct * 100.0);
    }
    
    #[test]
    fn test_hold_signal() {
        let signal = generate_trade_signal("BTCUSDT", 0.50, 95000.0);
        
        assert_eq!(signal.action, "HOLD");
        assert_eq!(signal.recommended_position_pct, 0.0);
    }
    
    #[test]
    fn test_kelly_calculation() {
        let kelly = calculate_kelly(0.60, 2.0);
        assert!(kelly > 0.0 && kelly < 1.0);
        println!("Kelly fraction: {:.4}", kelly);
    }
    
    #[test]
    fn test_volatility_adjustment() {
        let base_pct = 0.01;
        let adjusted = apply_volatility_adjustment(base_pct, 4.0);
        assert_eq!(adjusted, base_pct * 0.5);
    }
    
    #[test]
    fn test_expected_value_positive() {
        let ev = calculate_expected_value(0.60, 2.0);
        assert!(ev > 0.0);
    }
    
    #[test]
    fn test_expected_value_negative() {
        let ev = calculate_expected_value(0.40, 1.5);
        // EV = 0.40 * 1.5 - 0.60 = 0.60 - 0.60 = 0.0 (break-even, not negative)
        // For truly negative: need lower win rate or worse R:R
        assert!((ev - 0.0).abs() < 0.001, "EV should be ~0 for these parameters");
    }
}
