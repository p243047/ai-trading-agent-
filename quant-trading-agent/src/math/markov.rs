/// Markov regime transition matrix implementation
/// States: S0 (Range), S1 (Bull), S2 (Bear)

/// Transition probability matrix T[i][j] = P(S_j | S_i)
const TRANSITION_MATRIX: [[f64; 3]; 3] = [
    // From Range (S0) to: Range, Bull, Bear
    [0.60, 0.25, 0.15],
    // From Bull (S1) to: Range, Bull, Bear
    [0.20, 0.65, 0.15],
    // From Bear (S2) to: Range, Bull, Bear
    [0.25, 0.15, 0.60],
];

/// State indices
pub const STATE_RANGE: usize = 0;
pub const STATE_BULL: usize = 1;
pub const STATE_BEAR: usize = 2;

/// Calculate next state probabilities using transition matrix
pub fn calculate_next_state_probabilities(current_state: usize) -> [f64; 3] {
    if current_state > 2 {
        return [0.33, 0.34, 0.33]; // Default uniform distribution
    }
    
    TRANSITION_MATRIX[current_state]
}

/// Determine current Markov state from market metrics
pub fn determine_current_state(
    price_change_pct: f64,
    oi_change_pct: f64,
) -> usize {
    // Bullish: Price up + OI up or stable
    if price_change_pct > 1.0 && oi_change_pct >= -1.0 {
        STATE_BULL
    // Bearish: Price down + OI up (aggressive shorting)
    } else if price_change_pct < -1.0 && oi_change_pct > 0.0 {
        STATE_BEAR
    // Range: Low volatility in both dimensions
    } else if price_change_pct.abs() < 0.8 && oi_change_pct.abs() < 1.0 {
        STATE_RANGE
    // Strong bearish with liquidation
    } else if price_change_pct < -2.0 {
        STATE_BEAR
    // Strong bullish
    } else if price_change_pct > 2.0 {
        STATE_BULL
    } else {
        STATE_RANGE
    }
}

/// Calculate expected state after N transitions
pub fn predict_future_state_distribution(
    current_state: usize,
    steps: usize,
) -> [f64; 3] {
    let mut probs = calculate_next_state_probabilities(current_state);
    
    for _ in 1..steps {
        probs = multiply_vector_matrix(probs, TRANSITION_MATRIX);
    }
    
    probs
}

/// Multiply probability vector by transition matrix
fn multiply_vector_matrix(vec: [f64; 3], matrix: [[f64; 3]; 3]) -> [f64; 3] {
    let mut result = [0.0; 3];
    
    for j in 0..3 {
        for i in 0..3 {
            result[j] += vec[i] * matrix[i][j];
        }
    }
    
    result
}

/// Get the most likely future state
pub fn get_most_likely_state(probabilities: [f64; 3]) -> &'static str {
    let max_idx = probabilities
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
        .map(|(i, _)| i)
        .unwrap_or(STATE_RANGE);
    
    match max_idx {
        STATE_BULL => "BULL",
        STATE_BEAR => "BEAR",
        _ => "RANGE",
    }
}

/// Calculate regime persistence score (how likely to stay in current state)
pub fn calculate_persistence_score(state: usize) -> f64 {
    if state > 2 {
        return 0.5;
    }
    TRANSITION_MATRIX[state][state]
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_bull_state_transition() {
        let probs = calculate_next_state_probabilities(STATE_BULL);
        assert_eq!(probs.len(), 3);
        assert!(probs[STATE_BULL] > probs[STATE_RANGE]); // Most likely to stay bull
    }
    
    #[test]
    fn test_determine_state_bullish() {
        let state = determine_current_state(2.5, 3.0);
        assert_eq!(state, STATE_BULL);
    }
    
    #[test]
    fn test_determine_state_bearish() {
        let state = determine_current_state(-2.5, 4.0);
        assert_eq!(state, STATE_BEAR);
    }
    
    #[test]
    fn test_determine_state_range() {
        let state = determine_current_state(0.3, 0.5);
        assert_eq!(state, STATE_RANGE);
    }
    
    #[test]
    fn test_future_prediction() {
        let dist = predict_future_state_distribution(STATE_BULL, 3);
        assert!((dist.iter().sum::<f64>() - 1.0).abs() < 0.001); // Should sum to ~1
    }
    
    #[test]
    fn test_persistence_scores() {
        assert!(calculate_persistence_score(STATE_BULL) > 0.5);
        assert!(calculate_persistence_score(STATE_BEAR) > 0.5);
        assert!(calculate_persistence_score(STATE_RANGE) > 0.5);
    }
}
