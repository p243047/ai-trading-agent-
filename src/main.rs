mod types;
mod blackrock_tracker;
mod websocket_client;
mod local_ai;
mod math;
mod execution;

use types::{MarketMetrics, TradeSignal};
use std::error::Error;
use tokio::time::{sleep, Duration};
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt::init();
    info!("Starting Quantitative Local AI Trading Engine...");

    // 1. Initialize background scrapers and trackers
    let blackrock_flow_usd = blackrock_tracker::fetch_latest_etf_flows().await?;
    info!("Latest BlackRock Daily Net Flow: ${:.2}M", blackrock_flow_usd / 1_000_000.0);

    // 2. Target list of altcoins to monitor against BTC
    let target_coins = vec!["BTCUSDT", "ETHUSDT", "SOLUSDT", "AVAXUSDT", "LINKUSDT"];

    info!("Monitoring {} assets with 30-second evaluation cycles", target_coins.len());
    info!("Trade signals will be logged to paper_trades.json");
    info!("Press Ctrl+C to stop\n");

    loop {
        for symbol in &target_coins {
            // Fetch live WebSocket/REST snapshot from public exchange feeds
            let metrics = websocket_client::get_market_metrics(symbol, blackrock_flow_usd).await?;
            
            // Calculate quantitative probability
            let prob = math::bayesian::calculate_win_probability(&metrics);
            
            // Run local AI review for macro context sanity check
            let ai_verification = local_ai::review_trade_context(symbol, prob, &metrics).await?;

            // Apply AI confidence adjustment
            let final_prob = (prob + ai_verification.confidence_adjustment).clamp(0.0, 1.0);

            // Generate trade setup parameters
            let signal = math::kelly::generate_trade_signal(symbol, final_prob, metrics.current_price);

            // Display execution summary report
            if signal.action != "HOLD" {
                println!("\n==================================================");
                println!("    HIGH CONVICTION TRADE SIGNAL IDENTIFIED       ");
                println!("==================================================");
                println!(" Asset:              {}", signal.symbol);
                println!(" Direction:          {}", signal.action);
                println!(" Probability:        {:.2}%", signal.win_probability * 100.0);
                println!(" Current Price:      ${:.4}", signal.current_price);
                println!(" Target Price (TP):  ${:.4}", signal.target_price);
                println!(" Stop Loss (SL):     ${:.4}", signal.stop_loss);
                println!(" Position Size:      {:.2}% of account capital", signal.recommended_position_pct * 100.0);
                println!(" AI Rationality:     {}", ai_verification.reason);
                println!("==================================================\n");
                
                // Log the paper trade
                if let Err(e) = execution::log_paper_trade(&signal) {
                    warn!("Failed to log paper trade: {}", e);
                }
            } else {
                info!("Asset {} evaluated. Result: HOLD (Probability {:.2}%)", symbol, final_prob * 100.0);
            }
        }

        info!("Scanning cycle complete. Waiting for next evaluation window...");
        sleep(Duration::from_secs(30)).await;
    }
}
