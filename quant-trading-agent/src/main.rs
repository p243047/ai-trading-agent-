mod types;
mod blackrock_tracker;
mod websocket_client;
mod local_ai;
mod math;
mod execution;

use types::{MarketMetrics, TradeSignal};
use std::error::Error;
use std::collections::HashMap;
use tokio::time::{sleep, Duration};
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();
    
    info!("╔══════════════════════════════════════════════════════════╗");
    info!("║   QUANTITATIVE LOCAL AI TRADING ENGINE - PRODUCTION      ║");
    info!("║   Real-time Market Analysis & Paper Trading System       ║");
    info!("╚══════════════════════════════════════════════════════════╝");
    
    // Configuration
    const INITIAL_BALANCE: f64 = 100_000.0; // $100,000 paper trading account
    const SCAN_INTERVAL_SECS: u64 = 30;
    
    // Target assets to monitor
    let target_coins = vec![
        "BTCUSDT",
        "ETHUSDT", 
        "SOLUSDT",
        "AVAXUSDT",
        "LINKUSDT",
    ];
    
    info!("Monitoring {} assets with {}s scan interval", target_coins.len(), SCAN_INTERVAL_SECS);
    info!("Initial portfolio balance: ${:.2}", INITIAL_BALANCE);
    
    // Fetch BlackRock ETF flows once per cycle
    let blackrock_flow_usd = match blackrock_tracker::fetch_latest_etf_flows().await {
        Ok(flow) => {
            info!("✓ BlackRock IBIT Daily Net Flow: ${:.2}M", flow / 1_000_000.0);
            flow
        }
        Err(e) => {
            warn!("Failed to fetch ETF flows: {}", e);
            0.0
        }
    };
    
    // Track current prices for trade simulation
    let mut price_history: HashMap<String, Vec<f64>> = HashMap::new();
    
    // Main trading loop
    let mut cycle_count = 0;
    
    loop {
        cycle_count += 1;
        info!("\n════════════════════════════════════════════");
        info!("  SCAN CYCLE #{} - {}", cycle_count, chrono::Utc::now().format("%Y-%m-%d %H:%M:%S"));
        info!("════════════════════════════════════════════\n");
        
        for symbol in &target_coins {
            // Fetch live market data from exchanges
            let metrics = match websocket_client::get_market_metrics(symbol, blackrock_flow_usd).await {
                Ok(m) => m,
                Err(e) => {
                    warn!("Failed to get metrics for {}: {}", symbol, e);
                    continue;
                }
            };
            
            // Store price history
            price_history
                .entry(symbol.to_string())
                .or_insert_with(Vec::new)
                .push(metrics.current_price);
            
            // Keep only last 100 prices
            if let Some(history) = price_history.get_mut(*symbol) {
                if history.len() > 100 {
                    history.remove(0);
                }
            }
            
            // Calculate Bayesian win probability
            let base_probability = math::bayesian::calculate_win_probability(&metrics);
            
            // Determine market regime
            let regime = math::bayesian::determine_market_regime(&metrics);
            let markov_state = math::markov::determine_current_state(
                metrics.price_change_24h_pct,
                metrics.oi_change_24h_pct,
            );
            let state_name = match markov_state {
                math::markov::STATE_BULL => "BULL",
                math::markov::STATE_BEAR => "BEAR",
                _ => "RANGE",
            };
            
            // Run AI verification (or heuristic fallback)
            let ai_verification = match local_ai::review_trade_context(symbol, base_probability, &metrics).await {
                Ok(v) => v,
                Err(e) => {
                    warn!("AI review failed for {}: {}", symbol, e);
                    continue;
                }
            };
            
            // Apply AI confidence adjustment
            let final_probability = (base_probability + ai_verification.confidence_adjustment).clamp(0.0, 1.0);
            
            // Generate trade signal with Kelly sizing
            let signal = math::kelly::generate_trade_signal(symbol, final_probability, metrics.current_price);
            
            // Display analysis results
            println!("┌─────────────────────────────────────────────────────────┐");
            println!("│ ASSET: {:<10}                          MARKET: {:<8} │", symbol, regime);
            println!("├─────────────────────────────────────────────────────────┤");
            println!("│ Current Price:    ${:>12.4}", metrics.current_price);
            println!("│ 24h Change:       {:>+11.2}%", metrics.price_change_24h_pct);
            println!("│ Funding Rate:     {:>11.6}", metrics.funding_rate);
            println!("│ OI Change (24h):  {:>+11.2}%", metrics.oi_change_24h_pct);
            println!("│ Markov State:     {:>11}", state_name);
            println!("│ Win Probability:  {:>10.2}% → {:.2}% (AI adj)", 
                base_probability * 100.0, final_probability * 100.0);
            println!("│ AI Approved:      {:>11}", if ai_verification.approved { "YES" } else { "NO" });
            
            if signal.action != "HOLD" {
                // Execute paper trade
                let paper_trade = execution::execute_paper_trade(
                    &signal.symbol,
                    &signal.action,
                    &signal.market_type,
                    signal.entry_price,
                    signal.target_price,
                    signal.stop_loss,
                    signal.recommended_position_pct,
                );
                
                println!("│                                                             │");
                println!("│ ★★★★★ HIGH CONVICTION TRADE SIGNAL ★★★★★               │");
                println!("├─────────────────────────────────────────────────────────┤");
                println!("│ Direction:        {:>11} ({})", signal.action, signal.market_type);
                println!("│ Entry Price:      ${:>12.4}", signal.entry_price);
                println!("│ Take Profit (TP): ${:>12.4} (+{:.2}%)", 
                    signal.target_price,
                    (signal.target_price - signal.entry_price).abs() / signal.entry_price * 100.0);
                println!("│ Stop Loss (SL):   ${:>12.4} (-{:.2}%)", 
                    signal.stop_loss,
                    (signal.entry_price - signal.stop_loss).abs() / signal.entry_price * 100.0);
                println!("│ Position Size:    {:>11.4} ({:.2}% of capital)", 
                    format!("{:.2}%", signal.recommended_position_pct * 100.0),
                    signal.recommended_position_pct * 100.0);
                println!("│ Kelly Fraction:   {:>11.4}", signal.kelly_fraction);
                println!("│ Trade ID:         {:>11}", paper_trade.trade_id.split('-').next().unwrap_or("N/A"));
                println!("│ AI Rationale:     {}", truncate_str(&ai_verification.reason, 40));
                println!("└─────────────────────────────────────────────────────────┘\n");
            } else {
                println!("│ Status:           HOLD - No trade signal generated          │");
                println!("└─────────────────────────────────────────────────────────┘\n");
            }
        }
        
        // Check for trade closures based on current prices
        let current_prices: HashMap<String, f64> = target_coins
            .iter()
            .filter_map(|s| {
                price_history.get(*s)
                    .and_then(|h| h.last())
                    .map(|p| (s.to_string(), *p))
            })
            .collect();
        
        let closed_trades = execution::simulate_trade_closures(&current_prices);
        if !closed_trades.is_empty() {
            info!("Closed {} trades this cycle", closed_trades.len());
            for trade in &closed_trades {
                let result = if trade.success.unwrap_or(false) { "WIN" } else { "LOSS" };
                info!("  [{}] {} - PnL: {:.2}% (${:.2})", 
                    result, trade.symbol, trade.pnl_pct.unwrap_or(0.0), trade.pnl_usd.unwrap_or(0.0));
            }
        }
        
        // Print portfolio summary every 5 cycles
        if cycle_count % 5 == 0 {
            execution::print_portfolio_summary(INITIAL_BALANCE);
        }
        
        // Wait for next scan cycle
        info!("Waiting {} seconds for next scan...", SCAN_INTERVAL_SECS);
        sleep(Duration::from_secs(SCAN_INTERVAL_SECS)).await;
    }
}

/// Helper function to truncate strings
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len-3])
    }
}
