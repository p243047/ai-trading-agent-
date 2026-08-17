mod types;
mod blackrock_tracker;
mod websocket_client;
mod local_ai;
mod math;
mod execution;

use types::{MarketMetrics, TradeSignal, TradeType, Direction};
use std::error::Error;
use tokio::time::{sleep, Duration};
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt::init();
    info!("============================================================");
    info!("   QUANTITATIVE LOCAL AI TRADING ENGINE - STARTING UP");
    info!("============================================================");
    info!("Features:");
    info!("  • BlackRock ETF Flow Tracking (Institutional Signals)");
    info!("  • Real-time Market Data from Bybit/Binance APIs");
    info!("  • Bayesian Probability Scoring Engine");
    info!("  • Markov Regime Detection");
    info!("  • Kelly Criterion Position Sizing");
    info!("  • Spot & Futures Trading Support");
    info!("  • Paper Trading with PnL Tracking");
    info!("  • Success Rate Analytics");
    info!("============================================================\n");

    // 1. Initialize background scrapers and trackers
    let blackrock_flow_usd = match blackrock_tracker::fetch_latest_etf_flows().await {
        Ok(flow) => {
            info!("✅ BlackRock Daily Net Flow: ${:.2}M", flow / 1_000_000.0);
            flow
        },
        Err(e) => {
            warn!("⚠️  BlackRock tracker failed: {}. Using default.", e);
            0.0
        }
    };

    // 2. Target list of coins with full names
    let target_coins = vec![
        ("BTCUSDT", "Bitcoin"),
        ("ETHUSDT", "Ethereum"),
        ("SOLUSDT", "Solana"),
        ("AVAXUSDT", "Avalanche"),
        ("LINKUSDT", "Chainlink"),
    ];

    info!("Monitoring {} assets with 30-second evaluation cycles", target_coins.len());
    info!("Trade signals logged to paper_trades.json\n");
    info!("Press Ctrl+C to stop and view final statistics...\n");

    // Run initial cycle
    run_trading_cycle(&target_coins, blackrock_flow_usd).await?;

    // Continue in loop
    loop {
        sleep(Duration::from_secs(30)).await;
        run_trading_cycle(&target_coins, blackrock_flow_usd).await?;
    }
}

/// Run a single trading cycle
async fn run_trading_cycle(
    target_coins: &[(&str, &str)],
    blackrock_flow_usd: f64,
) -> Result<(), Box<dyn Error>> {
    info!("\n🔄 Starting new evaluation cycle...");
    
    let mut current_prices = Vec::new();
    let mut trade_signals = Vec::new();

    for (symbol, coin_name) in target_coins {
        // Fetch live market data from exchange APIs
        let metrics = match websocket_client::get_market_metrics(symbol, blackrock_flow_usd).await {
            Ok(m) => m,
            Err(e) => {
                warn!("⚠️  Failed to fetch metrics for {}: {}", symbol, e);
                continue;
            }
        };
        
        current_prices.push((symbol.to_string(), metrics.current_price));
        
        // Calculate quantitative probability using Bayesian engine
        let prob = math::bayesian::calculate_win_probability(&metrics);
        
        // Run local AI review (or fallback heuristic if Ollama unavailable)
        let ai_verification = local_ai::review_trade_context(symbol, prob, &metrics).await?;

        // Apply AI confidence adjustment
        let final_prob = (prob + ai_verification.confidence_adjustment).clamp(0.0, 1.0);

        // Determine trade type based on probability threshold
        let trade_type = if final_prob > 0.65 {
            TradeType::Futures  // Higher conviction → Futures
        } else if final_prob > 0.55 {
            TradeType::Spot     // Moderate conviction → Spot
        } else {
            TradeType::Spot     // Default to spot
        };

        // Generate trade setup parameters with Kelly sizing
        let mut signal = math::kelly::generate_trade_signal(symbol, final_prob, metrics.current_price);
        signal.coin_name = coin_name.to_string();
        signal.trade_type = trade_type;

        // Display and log trade signals
        if signal.action != "HOLD" {
            trade_signals.push(signal.clone());
            
            let trade_type_str = match signal.trade_type {
                TradeType::Spot => "(SPOT)",
                TradeType::Futures => "(FUTURES)",
            };
            
            println!("\n{}", "=".repeat(60));
            println!("   🎯 HIGH CONVICTION TRADE SIGNAL IDENTIFIED");
            println!("{}", "=".repeat(60));
            println!("   Asset:           {} {}", signal.coin_name, trade_type_str);
            println!("   Symbol:          {}", signal.symbol);
            println!("   Direction:       {}", signal.action);
            println!("   Win Probability: {:.2}%", signal.win_probability * 100.0);
            println!("   Entry Price:     ${:.4}", signal.current_price);
            println!("   Target (TP):     ${:.4}", signal.target_price);
            println!("   Stop Loss (SL):  ${:.4}", signal.stop_loss);
            println!("   Position Size:   {:.2}% of capital", signal.recommended_position_pct * 100.0);
            println!("   Kelly Fraction:  {:.3}", signal.kelly_fraction);
            println!("   AI Analysis:     {}", ai_verification.reason);
            println!("{}", "=".repeat(60));
            
            // Log the paper trade
            if let Err(e) = execution::log_paper_trade(&signal) {
                warn!("Failed to log paper trade: {}", e);
            }
        } else {
            info!("  • {}: HOLD (Prob: {:.1}%)", symbol, final_prob * 100.0);
        }
    }

    // Check and close existing open trades
    if !current_prices.is_empty() {
        if let Err(e) = execution::check_open_trades(&current_prices) {
            warn!("Error checking open trades: {}", e);
        }
        
        // Simulate some trade closures for demonstration
        if let Err(e) = execution::simulate_trade_closures(&current_prices) {
            warn!("Error simulating closures: {}", e);
        }
    }

    // Display performance statistics
    print_performance_summary()?;

    info!("✅ Cycle complete. Waiting for next evaluation window...");
    Ok(())
}

/// Print performance summary from closed trades
fn print_performance_summary() -> Result<(), Box<dyn Error>> {
    match execution::get_performance_stats() {
        Ok(stats) if stats.total_trades > 0 => {
            println!("\n{}", "-".repeat(50));
            println!("   📊 PERFORMANCE STATISTICS");
            println!("{}", "-".repeat(50));
            println!("   Total Trades:      {}", stats.total_trades);
            println!("   Winning Trades:    {}", stats.winning_trades);
            println!("   Losing Trades:     {}", stats.losing_trades);
            println!("   Win Rate:          {:.2}%", stats.win_rate * 100.0);
            println!("   Total PnL:         {:.2}%", stats.total_pnl_pct);
            println!("   Avg PnL per Trade: {:.2}%", stats.avg_pnl_pct);
            println!("{}", "-".repeat(50));
        },
        Ok(_) => {},
        Err(e) => warn!("Could not load performance stats: {}", e),
    }
    Ok(())
}
