//! Paper Execution & Trade Logging Module
//! 
//! Simulates trades locally and logs performance to JSON file

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use tracing::{info, warn};
use crate::types::{TradeSignal, PaperTrade as TradeRecord, TradeType, Direction};

/// Path to paper trades log file
const PAPER_TRADES_FILE: &str = "paper_trades.json";

/// Generate unique trade ID using UUID
fn generate_trade_id(symbol: &str) -> String {
    use uuid::Uuid;
    Uuid::new_v4().to_string()
}

/// Log a new paper trade to the JSON file
pub fn log_paper_trade(signal: &TradeSignal) -> Result<(), Box<dyn std::error::Error>> {
    let coin_name = signal.coin_name.clone();
    let trade_type = signal.trade_type;
    let direction = match signal.action.as_str() {
        "LONG" => Direction::Long,
        "SHORT" => Direction::Short,
        _ => return Ok(()), // Don't log HOLD signals
    };
    
    let mut trade = TradeRecord::new(
        &signal.symbol,
        &coin_name,
        trade_type,
        direction,
        &signal.action,
        signal.win_probability,
        signal.current_price,
        signal.target_price,
        signal.stop_loss,
        signal.recommended_position_pct,
        signal.kelly_fraction,
    );
    
    info!(
        "📝 Paper Trade Logged: {} {} {} @ ${:.2} (SL: ${:.2}, TP: ${:.2}, Prob: {:.1}%)",
        trade.action,
        trade.coin_name,
        match trade.trade_type {
            TradeType::Spot => "(SPOT)",
            TradeType::Futures => "(FUTURES)",
        },
        trade.entry_price,
        trade.stop_loss,
        trade.target_price,
        trade.win_probability * 100.0
    );
    
    // Load existing trades
    let mut trades = load_paper_trades()?;
    
    // Add new trade
    trades.push(trade);
    
    // Save updated list
    save_paper_trades(&trades)?;
    
    Ok(())
}

/// Update an existing paper trade (for simulating exits)
pub fn update_paper_trade(
    trade_id: &str,
    exit_price: f64,
    status: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut trades = load_paper_trades()?;
    
    for trade in &mut trades {
        if trade.trade_id == trade_id {
            trade.close_trade(exit_price, status);
            
            info!(
                "✅ Trade Closed: {} | PnL: {:.2}% | Status: {}",
                trade_id, 
                trade.pnl_pct.unwrap_or(0.0), 
                status
            );
            
            break;
        }
    }
    
    save_paper_trades(&trades)?;
    Ok(())
}

/// Calculate PnL percentage for a trade
fn calculate_pnl(direction: Direction, entry: f64, exit: f64) -> f64 {
    match direction {
        Direction::Long => ((exit - entry) / entry) * 100.0,
        Direction::Short => ((entry - exit) / entry) * 100.0,
    }
}

/// Load all paper trades from JSON file
pub fn load_paper_trades() -> Result<Vec<TradeRecord>, Box<dyn std::error::Error>> {
    if !Path::new(PAPER_TRADES_FILE).exists() {
        return Ok(Vec::new());
    }
    
    let mut file = File::open(PAPER_TRADES_FILE)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    
    if contents.trim().is_empty() {
        return Ok(Vec::new());
    }
    
    let trades: Vec<TradeRecord> = serde_json::from_str(&contents)?;
    Ok(trades)
}

/// Save paper trades to JSON file
fn save_paper_trades(trades: &[TradeRecord]) -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::to_string_pretty(trades)?;
    let mut file = File::create(PAPER_TRADES_FILE)?;
    file.write_all(json.as_bytes())?;
    Ok(())
}

/// Get performance statistics
pub fn get_performance_stats() -> Result<PerformanceStats, Box<dyn std::error::Error>> {
    let trades = load_paper_trades()?;
    
    let total_trades = trades.len();
    let closed_trades: Vec<&TradeRecord> = trades.iter()
        .filter(|t| t.status != "OPEN")
        .collect();
    
    let winning_trades = closed_trades.iter()
        .filter(|t| t.success.unwrap_or(false))
        .count();
    
    let losing_trades = closed_trades.iter()
        .filter(|t| !(t.success.unwrap_or(false)))
        .count();
    
    let total_pnl: f64 = closed_trades.iter()
        .map(|t| t.pnl_pct.unwrap_or(0.0))
        .sum();
    
    let avg_pnl = if closed_trades.is_empty() {
        0.0
    } else {
        total_pnl / closed_trades.len() as f64
    };
    
    let win_rate = if closed_trades.is_empty() {
        0.0
    } else {
        winning_trades as f64 / closed_trades.len() as f64
    };
    
    Ok(PerformanceStats {
        total_trades,
        winning_trades,
        losing_trades,
        win_rate,
        total_pnl_pct: total_pnl,
        avg_pnl_pct: avg_pnl,
    })
}

/// Performance statistics structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceStats {
    pub total_trades: usize,
    pub winning_trades: usize,
    pub losing_trades: usize,
    pub win_rate: f64,
    pub total_pnl_pct: f64,
    pub avg_pnl_pct: f64,
}

impl std::fmt::Display for PerformanceStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "=== Performance Statistics ===")?;
        writeln!(f, "Total Trades: {}", self.total_trades)?;
        writeln!(f, "Winning: {} | Losing: {}", self.winning_trades, self.losing_trades)?;
        writeln!(f, "Win Rate: {:.2}%", self.win_rate * 100.0)?;
        writeln!(f, "Total PnL: {:.2}%", self.total_pnl_pct)?;
        writeln!(f, "Avg PnL per Trade: {:.2}%", self.avg_pnl_pct)
    }
}

/// Simulate checking open trades against current prices and close some randomly for demo
pub fn check_open_trades(current_prices: &[(String, f64)]) -> Result<(), Box<dyn std::error::Error>> {
    let mut trades = load_paper_trades()?;
    let mut updated = false;
    
    // Create a price map
    let price_map: std::collections::HashMap<String, f64> = current_prices
        .iter()
        .map(|(s, p)| (s.clone(), *p))
        .collect();
    
    for trade in &mut trades {
        if trade.status != "OPEN" {
            continue;
        }
        
        // Find current price for this symbol
        if let Some(&price) = price_map.get(&trade.symbol) {
            // Check if stop loss hit
            if trade.direction == Direction::Long && price <= trade.stop_loss {
                trade.close_trade(price, "STOPPED");
                updated = true;
                info!("❌ Stop Loss Hit: {} @ ${:.2} | PnL: {:.2}%", 
                    trade.symbol, price, trade.pnl_pct.unwrap_or(0.0));
            } else if trade.direction == Direction::Short && price >= trade.stop_loss {
                trade.close_trade(price, "STOPPED");
                updated = true;
                info!("❌ Stop Loss Hit: {} @ ${:.2} | PnL: {:.2}%", 
                    trade.symbol, price, trade.pnl_pct.unwrap_or(0.0));
            }
            
            // Check if take profit hit
            if trade.direction == Direction::Long && price >= trade.target_price {
                trade.close_trade(price, "TARGET_HIT");
                updated = true;
                info!("🎯 Take Profit Hit: {} @ ${:.2} | PnL: {:.2}%", 
                    trade.symbol, price, trade.pnl_pct.unwrap_or(0.0));
            } else if trade.direction == Direction::Short && price <= trade.target_price {
                trade.close_trade(price, "TARGET_HIT");
                updated = true;
                info!("🎯 Take Profit Hit: {} @ ${:.2} | PnL: {:.2}%", 
                    trade.symbol, price, trade.pnl_pct.unwrap_or(0.0));
            }
        }
    }
    
    if updated {
        save_paper_trades(&trades)?;
    }
    
    Ok(())
}

/// Simulate closing some open trades with random price movements for demonstration
pub fn simulate_trade_closures(current_prices: &[(String, f64)]) -> Result<(), Box<dyn std::error::Error>> {
    let mut trades = load_paper_trades()?;
    let mut updated = false;
    
    // Create a price map
    let price_map: std::collections::HashMap<String, f64> = current_prices
        .iter()
        .map(|(s, p)| (s.clone(), *p))
        .collect();
    
    for trade in &mut trades {
        if trade.status != "OPEN" {
            continue;
        }
        
        if let Some(&base_price) = price_map.get(&trade.symbol) {
            // Simulate random price movement for demo purposes
            use std::time::{SystemTime, UNIX_EPOCH};
            let seed = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
            let random_factor = ((seed as f64 * 0.001).sin() + 1.0) / 2.0; // 0 to 1
            
            // 30% chance to close each cycle for demo
            if random_factor > 0.7 {
                let volatility = 0.02; // 2% volatility
                let simulated_move = (random_factor - 0.5) * volatility * 2.0;
                let exit_price = base_price * (1.0 + simulated_move);
                
                // Determine if TP or SL was hit first based on direction
                let (status, final_price) = if trade.direction == Direction::Long {
                    if exit_price >= trade.target_price {
                        ("TARGET_HIT", trade.target_price)
                    } else if exit_price <= trade.stop_loss {
                        ("STOPPED", trade.stop_loss)
                    } else {
                        ("SIMULATED_EXIT", exit_price)
                    }
                } else {
                    if exit_price <= trade.target_price {
                        ("TARGET_HIT", trade.target_price)
                    } else if exit_price >= trade.stop_loss {
                        ("STOPPED", trade.stop_loss)
                    } else {
                        ("SIMULATED_EXIT", exit_price)
                    }
                };
                
                trade.close_trade(final_price, status);
                updated = true;
                
                let emoji = match status {
                    "TARGET_HIT" => "🎯",
                    "STOPPED" => "❌",
                    _ => "📊",
                };
                
                info!("{} Trade Closed: {} {} | Entry: ${:.2} | Exit: ${:.2} | PnL: {:.2}%",
                    emoji, trade.action, trade.symbol, trade.entry_price, final_price, 
                    trade.pnl_pct.unwrap_or(0.0));
            }
        }
    }
    
    if updated {
        save_paper_trades(&trades)?;
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pnl_calculation_long() {
        let pnl = calculate_pnl(Direction::Long, 100.0, 110.0);
        assert!((pnl - 10.0).abs() < 0.01);
    }

    #[test]
    fn test_pnl_calculation_short() {
        let pnl = calculate_pnl(Direction::Short, 100.0, 90.0);
        assert!((pnl - 10.0).abs() < 0.01);
    }

    #[test]
    fn test_trade_id_generation() {
        let id = generate_trade_id("BTCUSDT");
        assert_eq!(id.len(), 36); // UUID length
    }
}
