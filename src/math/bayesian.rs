//! Bayesian Probability & Signal Scoring Engine
//! 
//! Implements logistic scoring model for win probability calculation

use crate::types::MarketMetrics;
use crate::blackrock_tracker::{calculate_blackrock_zscore, get_bias_score};
use crate::websocket_client::{calculate_funding_deviation, calculate_liquidity_imbalance};

/// Weights for the total signal score calculation
const W_BLACKROCK: f64 = 0.30;  // BlackRock flow weight
const W_DERIVATIVES: f64 = 0.25; // Derivatives momentum weight
const W_FUNDING: f64 = 0.20;     // Funding rate penalty weight
const W_LIQUIDITY: f64 = 0.15;   // Order book imbalance weight
const W_MARKOV: f64 = 0.10;      // Markov regime weight

/// Calculate overall win probability using Bayesian Logit Scoring Model
/// P_win = 1 / (1 + e^(-S_total))
/// 
/// Where S_total = w1*Z_BR + w2*(ΔOI * Sign(ΔP)) - w3*F_dev + w4*LI + w5*M_state
pub fn calculate_win_probability(metrics: &MarketMetrics) -> f64 {
    let s_total = calculate_total_signal_score(metrics);
    
    // Logistic function: P = 1 / (1 + e^(-S))
    let p_win = 1.0 / (1.0 + (-s_total).exp());
    
    p_win.clamp(0.0, 1.0)
}

/// Calculate the total signal score from all components
fn calculate_total_signal_score(metrics: &MarketMetrics) -> f64 {
    // 1. BlackRock Flow Z-Score component
    let br_zscore = calculate_blackrock_zscore(
        metrics.blackrock_flow_usd,
        metrics.blackrock_flow_mean_30d,
        metrics.blackrock_flow_std_30d,
    );
    let br_component = get_bias_score(br_zscore);
    
    // 2. Derivatives Momentum (OI change * price direction sign)
    let oi_momentum = metrics.open_interest_24h_change;
    let price_sign = if metrics.current_price > 0.0 { 1.0 } else { -1.0 };
    let derivatives_component = oi_momentum * price_sign * 10.0; // Scale factor
    
    // 3. Funding Rate Deviation (penalty for extreme funding)
    let f_dev = calculate_funding_deviation(
        metrics.funding_rate,
        metrics.funding_rate_avg_7d,
        metrics.funding_rate_std_7d,
    );
    let funding_component = f_dev;
    
    // 4. Liquidity Imbalance
    let li = calculate_liquidity_imbalance(
        metrics.bid_volume_1pct,
        metrics.ask_volume_1pct,
    );
    let liquidity_component = li * 5.0; // Scale factor
    
    // 5. Markov Regime State (simplified - based on price trend)
    let markov_component = infer_markov_state(metrics);
    
    // Combine all components with weights
    let s_total = 
        W_BLACKROCK * br_component +
        W_DERIVATIVES * derivatives_component -
        W_FUNDING * funding_component +
        W_LIQUIDITY * liquidity_component +
        W_MARKOV * markov_component;
    
    s_total
}

/// Infer Markov regime state from market metrics
/// Returns: +2.0 for Bullish, -2.0 for Bearish, 0.0 for Range-bound
fn infer_markov_state(metrics: &MarketMetrics) -> f64 {
    // Simple heuristic: positive OI change with positive price momentum = bullish
    if metrics.open_interest_24h_change > 0.05 {
        2.0 // Strong bullish
    } else if metrics.open_interest_24h_change < -0.05 {
        -2.0 // Strong bearish
    } else {
        0.0 // Range-bound
    }
}

/// Get trade direction recommendation based on probability
pub fn get_direction(probability: f64) -> &'static str {
    const LONG_THRESHOLD: f64 = 0.55;
    const SHORT_THRESHOLD: f64 = 0.45;
    
    if probability >= LONG_THRESHOLD {
        "LONG"
    } else if probability <= SHORT_THRESHOLD {
        "SHORT"
    } else {
        "HOLD"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_logistic_function() {
        // Test that probability is bounded between 0 and 1
        let mut metrics = MarketMetrics::new("BTCUSDT");
        metrics.blackrock_flow_usd = 150_000_000.0;
        metrics.open_interest_24h_change = 0.10;
        
        let prob = calculate_win_probability(&metrics);
        assert!(prob >= 0.0 && prob <= 1.0);
    }

    #[test]
    fn test_direction_thresholds() {
        assert_eq!(get_direction(0.60), "LONG");
        assert_eq!(get_direction(0.40), "SHORT");
        assert_eq!(get_direction(0.50), "HOLD");
    }
}
