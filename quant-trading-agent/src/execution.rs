use crate::types::{PaperTrade, Portfolio};
use chrono::Utc;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use uuid::Uuid;
use tracing::info;

const PAPER_TRADES_FILE: &str = "paper_trades.json";

/// Execute a paper trade and log it
pub fn execute_paper_trade(
    symbol: &str,
    action: &str,
    market_type: &str,
    entry_price: f64,
    target_price: f64,
    stop_loss: f64,
    position_size_pct: f64,
) -> PaperTrade {
    let trade = PaperTrade {
        trade_id: Uuid::new_v4().to_string(),
        symbol: symbol.to_string(),
        action: action.to_string(),
        market_type: market_type.to_string(),
        entry_price,
        entry_time: Utc::now(),
        target_price,
        stop_loss,
        position_size_pct,
        status: "OPEN".to_string(),
        exit_price: None,
        exit_time: None,
        pnl_pct: None,
        pnl_usd: None,
        success: None,
    };
    
    // Save to file
    save_trade(&trade);
    
    info!(
        "PAPER TRADE EXECUTED: {} {} @ ${} (TP: ${}, SL: ${}, Size: {:.2}%)",
        action, symbol, entry_price, target_price, stop_loss, position_size_pct * 100.0
    );
    
    trade
}

/// Close a paper trade with simulated exit
pub fn close_paper_trade(
    trade_id: &str,
    exit_price: f64,
    account_balance: f64,
) -> Option<PaperTrade> {
    let mut trades = load_all_trades();
    let mut result_trade = None;
    
    for trade in trades.iter_mut() {
        if trade.trade_id == trade_id && trade.status == "OPEN" {
            // Calculate PnL
            let pnl_pct = calculate_pnl_pct(
                trade.entry_price,
                exit_price,
                &trade.action,
            );
            
            let pnl_usd = account_balance * trade.position_size_pct * pnl_pct;
            let is_winner = pnl_pct > 0.0;
            
            trade.exit_price = Some(exit_price);
            trade.exit_time = Some(Utc::now());
            trade.pnl_pct = Some(pnl_pct * 100.0); // As percentage
            trade.pnl_usd = Some(pnl_usd);
            trade.success = Some(is_winner);
            trade.status = "CLOSED".to_string();
            
            result_trade = Some(trade.clone());
            break;
        }
    }
    
    // Save updated trades after the mutable borrow ends
    if result_trade.is_some() {
        save_all_trades(&trades);
    }
    
    result_trade
}

fn calculate_pnl_pct(entry: f64, exit: f64, action: &str) -> f64 {
    match action {
        "LONG" => (exit - entry) / entry,
        "SHORT" => (entry - exit) / entry,
        _ => 0.0,
    }
}

/// Save single trade to file
fn save_trade(trade: &PaperTrade) {
    let mut trades = load_all_trades();
    trades.push(trade.clone());
    save_all_trades(&trades);
}

/// Load all trades from file
pub fn load_all_trades() -> Vec<PaperTrade> {
    if let Ok(mut file) = File::open(PAPER_TRADES_FILE) {
        let mut contents = String::new();
        if file.read_to_string(&mut contents).is_ok() {
            if let Ok(trades) = serde_json::from_str::<Vec<PaperTrade>>(&contents) {
                return trades;
            }
        }
    }
    Vec::new()
}

/// Save all trades to file
fn save_all_trades(trades: &[PaperTrade]) {
    let trades_clone = trades.to_vec();
    if let Ok(file) = File::create(PAPER_TRADES_FILE) {
        let mut buffered = std::io::BufWriter::new(file);
        if let Ok(json) = serde_json::to_string_pretty(&trades_clone) {
            let _ = buffered.write_all(json.as_bytes());
        }
    }
}

/// Calculate portfolio statistics from trade history
pub fn calculate_portfolio_stats(initial_balance: f64) -> Portfolio {
    let trades = load_all_trades();
    let mut portfolio = Portfolio::new(initial_balance);
    
    for trade in &trades {
        if trade.status == "CLOSED" {
            let pnl_usd = trade.pnl_usd.unwrap_or(0.0);
            let is_winner = trade.success.unwrap_or(false);
            portfolio.update_stats(pnl_usd, is_winner);
        }
    }
    
    portfolio
}

/// Simulate closing some open trades for demonstration
pub fn simulate_trade_closures(current_prices: &std::collections::HashMap<String, f64>) -> Vec<PaperTrade> {
    let mut trades = load_all_trades();
    let mut closed_trades = Vec::new();
    
    for trade in trades.iter_mut() {
        if trade.status == "OPEN" {
            if let Some(current_price) = current_prices.get(&trade.symbol) {
                // Check if TP or SL hit
                let should_close = match trade.action.as_str() {
                    "LONG" => {
                        *current_price >= trade.target_price || *current_price <= trade.stop_loss
                    }
                    "SHORT" => {
                        *current_price <= trade.target_price || *current_price >= trade.stop_loss
                    }
                    _ => false,
                };
                
                if should_close {
                    let exit_price = *current_price;
                    let pnl_pct = calculate_pnl_pct(trade.entry_price, exit_price, &trade.action);
                    let pnl_usd = 100000.0 * trade.position_size_pct * pnl_pct; // Assume $100k account
                    
                    trade.exit_price = Some(exit_price);
                    trade.exit_time = Some(Utc::now());
                    trade.pnl_pct = Some(pnl_pct * 100.0);
                    trade.pnl_usd = Some(pnl_usd);
                    trade.success = Some(pnl_pct > 0.0);
                    trade.status = "CLOSED".to_string();
                    
                    closed_trades.push(trade.clone());
                }
            }
        }
    }
    
    if !closed_trades.is_empty() {
        save_all_trades(&trades);
    }
    
    closed_trades
}

/// Print portfolio summary
pub fn print_portfolio_summary(initial_balance: f64) {
    let portfolio = calculate_portfolio_stats(initial_balance);
    
    println!("\n╔══════════════════════════════════════════════════════════╗");
    println!("║              PORTFOLIO PERFORMANCE SUMMARY             ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║ Initial Balance:     ${:>12.2}", portfolio.initial_balance);
    println!("║ Current Balance:     ${:>12.2}", portfolio.current_balance);
    println!("║ Total PnL:           ${:>12.2}", portfolio.current_balance - portfolio.initial_balance);
    println!("║                                                            ");
    println!("║ Total Trades:        {:>12}", portfolio.total_trades);
    println!("║ Winning Trades:      {:>12}", portfolio.winning_trades);
    println!("║ Losing Trades:       {:>12}", portfolio.losing_trades);
    println!("║ Win Rate:            {:>11.2}%", portfolio.win_rate * 100.0);
    println!("╚══════════════════════════════════════════════════════════╝\n");
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_execute_paper_trade() {
        let trade = execute_paper_trade(
            "BTCUSDT",
            "LONG",
            "FUTURES",
            95000.0,
            98800.0,
            93100.0,
            0.01,
        );
        
        assert_eq!(trade.symbol, "BTCUSDT");
        assert_eq!(trade.action, "LONG");
        assert_eq!(trade.status, "OPEN");
        assert!(trade.trade_id.len() > 0);
    }
    
    #[test]
    fn test_pnl_calculation_long() {
        let pnl = calculate_pnl_pct(100.0, 110.0, "LONG");
        assert!((pnl - 0.10).abs() < 0.001);
    }
    
    #[test]
    fn test_pnl_calculation_short() {
        let pnl = calculate_pnl_pct(100.0, 90.0, "SHORT");
        assert!((pnl - 0.10).abs() < 0.001);
    }
}
