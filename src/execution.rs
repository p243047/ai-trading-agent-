//! Paper Execution & Trade Logging Module
//! 
//! Simulates trades locally and logs performance to JSON file

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use tracing::{info, warn};
use crate::types::TradeSignal;

/// Path to paper trades log file
const PAPER_TRADES_FILE: &str = "paper_trades.json";

/// Paper trade record structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaperTrade {
    pub id: String,
    pub symbol: String,
    pub direction: String,
    pub entry_price: f64,
    pub stop_loss: f64,
    pub take_profit: f64,
    pub position_size_pct: f64,
    pub win_probability: f64,
    pub entry_time: DateTime<Utc>,
    pub exit_time: Option<DateTime<Utc>>,
    pub exit_price: Option<f64>,
    pub pnl_pct: Option<f64>,
    pub status: String, // "OPEN", "CLOSED", "STOPPED_OUT", "TP_HIT"
}

impl PaperTrade {
    pub fn from_signal(signal: &TradeSignal) -> Self {
        PaperTrade {
            id: generate_trade_id(&signal.symbol),
            symbol: signal.symbol.clone(),
            direction: signal.action.clone(),
            entry_price: signal.current_price,
            stop_loss: signal.stop_loss,
            take_profit: signal.target_price,
            position_size_pct: signal.recommended_position_pct,
            win_probability: signal.win_probability,
            entry_time: Utc::now(),
            exit_time: None,
            exit_price: None,
            pnl_pct: None,
            status: "OPEN".to_string(),
        }
    }
}

/// Generate unique trade ID
fn generate_trade_id(symbol: &str) -> String {
    let timestamp = Utc::now().timestamp();
    format!("{}_{}", symbol, timestamp)
}

/// Log a new paper trade to the JSON file
pub fn log_paper_trade(signal: &TradeSignal) -> Result<(), Box<dyn std::error::Error>> {
    let mut trade = PaperTrade::from_signal(signal);
    
    info!(
        "📝 Paper Trade Logged: {} {} @ ${:.2} (SL: ${:.2}, TP: ${:.2})",
        trade.direction,
        trade.symbol,
        trade.entry_price,
        trade.stop_loss,
        trade.take_profit
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
        if trade.id == trade_id {
            trade.exit_time = Some(Utc::now());
            trade.exit_price = Some(exit_price);
            trade.status = status.to_string();
            
            // Calculate PnL
            let pnl = calculate_pnl(
                &trade.direction,
                trade.entry_price,
                exit_price,
            );
            trade.pnl_pct = Some(pnl);
            
            info!(
                "✅ Trade Closed: {} | PnL: {:.2}% | Status: {}",
                trade_id, pnl, status
            );
            
            break;
        }
    }
    
    save_paper_trades(&trades)?;
    Ok(())
}

/// Calculate PnL percentage for a trade
fn calculate_pnl(direction: &str, entry: f64, exit: f64) -> f64 {
    match direction {
        "LONG" => ((exit - entry) / entry) * 100.0,
        "SHORT" => ((entry - exit) / entry) * 100.0,
        _ => 0.0,
    }
}

/// Load all paper trades from JSON file
pub fn load_paper_trades() -> Result<Vec<PaperTrade>, Box<dyn std::error::Error>> {
    if !Path::new(PAPER_TRADES_FILE).exists() {
        return Ok(Vec::new());
    }
    
    let mut file = File::open(PAPER_TRADES_FILE)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    
    if contents.trim().is_empty() {
        return Ok(Vec::new());
    }
    
    let trades: Vec<PaperTrade> = serde_json::from_str(&contents)?;
    Ok(trades)
}

/// Save paper trades to JSON file
fn save_paper_trades(trades: &[PaperTrade]) -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::to_string_pretty(trades)?;
    let mut file = File::create(PAPER_TRADES_FILE)?;
    file.write_all(json.as_bytes())?;
    Ok(())
}

/// Get performance statistics
pub fn get_performance_stats() -> Result<PerformanceStats, Box<dyn std::error::Error>> {
    let trades = load_paper_trades()?;
    
    let total_trades = trades.len();
    let closed_trades: Vec<&PaperTrade> = trades.iter()
        .filter(|t| t.status != "OPEN")
        .collect();
    
    let winning_trades = closed_trades.iter()
        .filter(|t| t.pnl_pct.unwrap_or(0.0) > 0.0)
        .count();
    
    let losing_trades = closed_trades.iter()
        .filter(|t| t.pnl_pct.unwrap_or(0.0) <= 0.0)
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

/// Simulate checking open trades against current prices
pub fn check_open_trades(current_prices: &[(&str, f64)]) -> Result<(), Box<dyn std::error::Error>> {
    let mut trades = load_paper_trades()?;
    let mut updated = false;
    
    for trade in &mut trades {
        if trade.status != "OPEN" {
            continue;
        }
        
        // Find current price for this symbol
        let current_price = current_prices.iter()
            .find(|(symbol, _)| *symbol == trade.symbol)
            .map(|(_, price)| *price);
        
        if let Some(price) = current_price {
            // Check if stop loss hit
            if trade.direction == "LONG" && price <= trade.stop_loss {
                trade.exit_time = Some(Utc::now());
                trade.exit_price = Some(price);
                trade.pnl_pct = Some(calculate_pnl(&trade.direction, trade.entry_price, price));
                trade.status = "STOPPED_OUT".to_string();
                updated = true;
                info!("❌ Stop Loss Hit: {} @ ${:.2}", trade.symbol, price);
            } else if trade.direction == "SHORT" && price >= trade.stop_loss {
                trade.exit_time = Some(Utc::now());
                trade.exit_price = Some(price);
                trade.pnl_pct = Some(calculate_pnl(&trade.direction, trade.entry_price, price));
                trade.status = "STOPPED_OUT".to_string();
                updated = true;
                info!("❌ Stop Loss Hit: {} @ ${:.2}", trade.symbol, price);
            }
            
            // Check if take profit hit
            if trade.direction == "LONG" && price >= trade.take_profit {
                trade.exit_time = Some(Utc::now());
                trade.exit_price = Some(price);
                trade.pnl_pct = Some(calculate_pnl(&trade.direction, trade.entry_price, price));
                trade.status = "TP_HIT".to_string();
                updated = true;
                info!("🎯 Take Profit Hit: {} @ ${:.2}", trade.symbol, price);
            } else if trade.direction == "SHORT" && price <= trade.take_profit {
                trade.exit_time = Some(Utc::now());
                trade.exit_price = Some(price);
                trade.pnl_pct = Some(calculate_pnl(&trade.direction, trade.entry_price, price));
                trade.status = "TP_HIT".to_string();
                updated = true;
                info!("🎯 Take Profit Hit: {} @ ${:.2}", trade.symbol, price);
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
        let pnl = calculate_pnl("LONG", 100.0, 110.0);
        assert!((pnl - 10.0).abs() < 0.01);
    }

    #[test]
    fn test_pnl_calculation_short() {
        let pnl = calculate_pnl("SHORT", 100.0, 90.0);
        assert!((pnl - 10.0).abs() < 0.01);
    }

    #[test]
    fn test_trade_id_generation() {
        let id = generate_trade_id("BTCUSDT");
        assert!(id.starts_with("BTCUSDT_"));
    }
}
