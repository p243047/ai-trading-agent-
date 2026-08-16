//! Markov Regime Transition Engine
//! 
//! Implements state transition matrix for market regime detection

use crate::types::MarketRegime;
use ndarray::{Array2, s};

/// Market state transition matrix T
/// T[i][j] = P(S_j | S_i) - probability of transitioning from state i to state j
/// States: 0 = RangeBound, 1 = Bullish, 2 = Bearish
pub struct MarkovTransitionMatrix {
    matrix: Array2<f64>,
}

impl MarkovTransitionMatrix {
    /// Create a new transition matrix with default probabilities
    /// These should be calibrated from historical data in production
    pub fn new() -> Self {
        // Default transition probabilities (rows must sum to 1.0)
        // [P(S0|S0), P(S1|S0), P(S2|S0)]
        // [P(S0|S1), P(S1|S1), P(S2|S1)]
        // [P(S0|S2), P(S1|S2), P(S2|S2)]
        let matrix = Array2::from_shape_vec((3, 3), vec![
            0.50, 0.30, 0.20,  // From RangeBound
            0.20, 0.60, 0.20,  // From Bullish
            0.25, 0.25, 0.50,  // From Bearish
        ]).unwrap();
        
        MarkovTransitionMatrix { matrix }
    }
    
    /// Get transition probability from one state to another
    pub fn transition_prob(&self, from: MarketRegime, to: MarketRegime) -> f64 {
        let from_idx = regime_to_index(from);
        let to_idx = regime_to_index(to);
        self.matrix[[from_idx, to_idx]]
    }
    
    /// Predict next state distribution given current state
    pub fn predict_next_state(&self, current_state: MarketRegime) -> [f64; 3] {
        let from_idx = regime_to_index(current_state);
        [
            self.matrix[[from_idx, 0]],
            self.matrix[[from_idx, 1]],
            self.matrix[[from_idx, 2]],
        ]
    }
    
    /// Calculate steady-state (equilibrium) distribution
    /// Solves: π * T = π where π is the stationary distribution
    pub fn steady_state_distribution(&self) -> [f64; 3] {
        // Simplified power iteration method
        let mut dist = [0.33, 0.34, 0.33]; // Initial uniform distribution
        
        for _ in 0..100 {
            let new_dist = [
                dist[0] * self.matrix[[0, 0]] + dist[1] * self.matrix[[1, 0]] + dist[2] * self.matrix[[2, 0]],
                dist[0] * self.matrix[[0, 1]] + dist[1] * self.matrix[[1, 1]] + dist[2] * self.matrix[[2, 1]],
                dist[0] * self.matrix[[0, 2]] + dist[1] * self.matrix[[1, 2]] + dist[2] * self.matrix[[2, 2]],
            ];
            dist = new_dist;
        }
        
        dist
    }
    
    /// Update transition matrix with new observed transitions
    pub fn update_from_observation(&mut self, from: MarketRegime, to: MarketRegime, learning_rate: f64) {
        let from_idx = regime_to_index(from);
        let to_idx = regime_to_index(to);
        
        // Simple Bayesian update with learning rate
        let current_prob = self.matrix[[from_idx, to_idx]];
        self.matrix[[from_idx, to_idx]] = current_prob + learning_rate * (1.0 - current_prob);
        
        // Normalize row to ensure probabilities sum to 1
        self.normalize_row(from_idx);
    }
    
    fn normalize_row(&mut self, row: usize) {
        let row_sum: f64 = (0..3).map(|col| self.matrix[[row, col]]).sum();
        if row_sum > 0.0 {
            for col in 0..3 {
                self.matrix[[row, col]] /= row_sum;
            }
        }
    }
}

impl Default for MarkovTransitionMatrix {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert MarketRegime enum to matrix index
fn regime_to_index(regime: MarketRegime) -> usize {
    match regime {
        MarketRegime::RangeBound => 0,
        MarketRegime::Bullish => 1,
        MarketRegime::Bearish => 2,
    }
}

/// Convert matrix index to MarketRegime enum
pub fn index_to_regime(index: usize) -> Option<MarketRegime> {
    match index {
        0 => Some(MarketRegime::RangeBound),
        1 => Some(MarketRegime::Bullish),
        2 => Some(MarketRegime::Bearish),
        _ => None,
    }
}

/// Determine current market regime from price and momentum indicators
pub fn detect_market_regime(
    price_change_pct: f64,
    volatility: f64,
    trend_strength: f64,
) -> MarketRegime {
    // Heuristic regime detection
    if price_change_pct.abs() < 2.0 && volatility < 0.03 {
        MarketRegime::RangeBound
    } else if price_change_pct > 2.0 && trend_strength > 0.5 {
        MarketRegime::Bullish
    } else if price_change_pct < -2.0 && trend_strength < -0.5 {
        MarketRegime::Bearish
    } else {
        MarketRegime::RangeBound
    }
}

/// Get regime score for Bayesian calculation
/// Returns: +2.0 for Bullish, -2.0 for Bearish, 0.0 for Range-bound
pub fn get_regime_score(regime: MarketRegime) -> f64 {
    match regime {
        MarketRegime::Bullish => 2.0,
        MarketRegime::Bearish => -2.0,
        MarketRegime::RangeBound => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transition_matrix_creation() {
        let matrix = MarkovTransitionMatrix::new();
        // Row sums should equal 1.0
        for row in 0..3 {
            let row_sum: f64 = (0..3).map(|col| matrix.matrix[[row, col]]).sum();
            assert!((row_sum - 1.0).abs() < 0.001);
        }
    }

    #[test]
    fn test_steady_state() {
        let matrix = MarkovTransitionMatrix::new();
        let steady = matrix.steady_state_distribution();
        let sum: f64 = steady.iter().sum();
        assert!((sum - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_regime_detection() {
        let regime = detect_market_regime(3.0, 0.02, 0.7);
        assert_eq!(regime, MarketRegime::Bullish);
        
        let regime = detect_market_regime(-3.0, 0.02, -0.7);
        assert_eq!(regime, MarketRegime::Bearish);
        
        let regime = detect_market_regime(1.0, 0.01, 0.1);
        assert_eq!(regime, MarketRegime::RangeBound);
    }
}
