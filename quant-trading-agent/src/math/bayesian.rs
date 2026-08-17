use crate::types::MarketMetrics;

/// Calculate Bayesian win probability using logistic function
pub fn calculate_win_probability(metrics: &MarketMetrics) -> f64 {
    let total_score = calculate_total_signal_score(metrics);
    
    // Logistic function: P_win = 1 / (1 + e^(-S_total))
    let probability = 1.0 / (1.0 + (-total_score).exp());
    
    probability.clamp(0.0, 1.0)
}

/// Calculate total signal score S_total
/// S_total = w1*Z_BR + w2*(ΔOI * Sign(ΔP)) - w3*F_dev + w4*LI + w5*M_state
fn calculate_total_signal_score(metrics: &MarketMetrics) -> f64 {
    // Weights as specified in the spec
    let w1 = 0.30; // BlackRock flow weight
    let w2 = 0.25; // Derivatives momentum weight
    let w3 = 0.20; // Funding rate penalty weight
    let w4 = 0.15; // Order book imbalance weight
    let w5 = 0.10; // Markov regime weight
    
    // Z_BR: BlackRock flow Z-score
    let br_zscore = calculate_br_zscore_component(metrics.blackrock_flow_usd);
    
    // ΔOI * Sign(ΔP): OI change with price direction
    let oi_momentum = metrics.oi_change_24h_pct * metrics.price_change_24h_pct.signum();
    
    // F_dev: Funding rate deviation (normalized)
    let funding_deviation = metrics.funding_rate * 1000.0; // Scale for comparison
    
    // LI: Liquidity imbalance
    let liquidity_imbalance = metrics.liquidity_imbalance();
    
    // M_state: Markov state score (simplified based on price trend)
    let markov_state = calculate_markov_state_score(metrics.price_change_24h_pct);
    
    w1 * br_zscore + w2 * oi_momentum - w3 * funding_deviation + w4 * liquidity_imbalance + w5 * markov_state
}

/// Calculate BlackRock Z-score component from flow data
fn calculate_br_zscore_component(flow_usd: f64) -> f64 {
    // Normalize flow to millions
    let flow_millions = flow_usd / 1_000_000.0;
    
    // Using historical mean and std from blackrock_tracker
    let historical_mean = 120.0; // Average daily flow in millions
    let historical_std = 150.0;  // Standard deviation
    
    if historical_std == 0.0 {
        return 0.0;
    }
    
    (flow_millions - historical_mean) / historical_std
}

/// Calculate Markov state score based on price trend
/// States: S0 (Range), S1 (Bull), S2 (Bear)
fn calculate_markov_state_score(price_change_pct: f64) -> f64 {
    if price_change_pct > 2.0 {
        1.5 // Strong bullish state
    } else if price_change_pct > 0.5 {
        0.8 // Moderate bullish
    } else if price_change_pct < -2.0 {
        -1.5 // Strong bearish state
    } else if price_change_pct < -0.5 {
        -0.8 // Moderate bearish
    } else {
        0.0 // Range-bound
    }
}

/// Determine market regime based on multiple indicators
pub fn determine_market_regime(metrics: &MarketMetrics) -> &'static str {
    let price_momentum = metrics.price_change_24h_pct;
    let oi_momentum = metrics.oi_change_24h_pct;
    
    // Bullish: Price up + OI up (strong expansion)
    if price_momentum > 1.0 && oi_momentum > 0.0 {
        "BULLISH"
    // Bearish: Price down + OI up (aggressive shorting)
    } else if price_momentum < -1.0 && oi_momentum > 0.0 {
        "BEARISH"
    // Liquidation: Price down + OI down (long squeeze)
    } else if price_momentum < -2.0 && oi_momentum < 0.0 {
        "LIQUIDATION"
    // Range: Low volatility
    } else if price_momentum.abs() < 0.5 {
        "RANGE"
    } else {
        "NEUTRAL"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::MarketMetrics;
    use chrono::Utc;
    
    fn create_test_metrics() -> MarketMetrics {
        MarketMetrics {
            symbol: "BTCUSDT".to_string(),
            current_price: 95000.0,
            funding_rate: 0.0001,
            open_interest: 5000000000.0,
            oi_change_24h_pct: 2.5,
            price_change_24h_pct: 1.8,
            bid_volume_1pct: 750000000.0,
            ask_volume_1pct: 600000000.0,
            blackrock_flow_usd: 200_000_000.0,
            timestamp: Utc::now(),
        }
    }
    
    #[test]
    fn test_probability_calculation() {
        let metrics = create_test_metrics();
        let prob = calculate_win_probability(&metrics);
        
        assert!(prob >= 0.0 && prob <= 1.0, "Probability must be between 0 and 1");
        println!("Win probability: {:.2}%", prob * 100.0);
    }
    
    #[test]
    fn test_market_regime_bullish() {
        let mut metrics = create_test_metrics();
        metrics.price_change_24h_pct = 3.0;
        metrics.oi_change_24h_pct = 5.0;
        
        let regime = determine_market_regime(&metrics);
        assert_eq!(regime, "BULLISH");
    }
    
    #[test]
    fn test_market_regime_bearish() {
        let mut metrics = create_test_metrics();
        metrics.price_change_24h_pct = -3.0;
        metrics.oi_change_24h_pct = 4.0;
        
        let regime = determine_market_regime(&metrics);
        assert_eq!(regime, "BEARISH");
    }
    
    #[test]
    fn test_market_regime_liquidation() {
        let mut metrics = create_test_metrics();
        metrics.price_change_24h_pct = -4.0;
        metrics.oi_change_24h_pct = -3.0;
        
        let regime = determine_market_regime(&metrics);
        assert_eq!(regime, "LIQUIDATION");
    }
    
    #[test]
    fn test_market_regime_range() {
        let mut metrics = create_test_metrics();
        metrics.price_change_24h_pct = 0.3;
        metrics.oi_change_24h_pct = 0.2;
        
        let regime = determine_market_regime(&metrics);
        assert_eq!(regime, "RANGE");
    }
}
