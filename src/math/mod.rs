//! Math Module
//! 
//! Contains quantitative engines for probability scoring, regime detection, and position sizing

pub mod bayesian;
pub mod markov;
pub mod kelly;

// Re-export commonly used functions
pub use bayesian::calculate_win_probability;
pub use bayesian::get_direction;
pub use markov::{MarkovTransitionMatrix, detect_market_regime, get_regime_score};
pub use kelly::generate_trade_signal;
