use tokio_tungstenite::{connect_async, tungstenite::Message};
use futures_util::{StreamExt, SinkExt};
use serde_json::Value;
use tracing::{info, warn, error};
use crate::types::MarketMetrics;

/// Bybit WebSocket base URL for futures
const BYBIT_WS_URL: &str = "wss://stream.bybit.com/v5/public/linear";

/// Binance WebSocket base URL for futures
const BINANCE_WS_URL: &str = "wss://fstream.binance.com/ws";

/// Fetch market metrics for a given symbol using WebSocket or REST fallback
pub async fn get_market_metrics(symbol: &str, blackrock_flow_usd: f64) -> Result<MarketMetrics, Box<dyn std::error::Error>> {
    let mut metrics = MarketMetrics::new(symbol);
    metrics.blackrock_flow_usd = blackrock_flow_usd;
    
    // Set default historical values (in production, these would be tracked over time)
    metrics.blackrock_flow_mean_30d = 50_000_000.0; // $50M average
    metrics.blackrock_flow_std_30d = 75_000_000.0;  // $75M std dev
    
    // Try Bybit REST API first
    match fetch_metrics_bybit_rest(symbol).await {
        Ok(rest_metrics) => {
            metrics.current_price = rest_metrics.current_price;
            metrics.open_interest = rest_metrics.open_interest;
            metrics.open_interest_24h_change = rest_metrics.open_interest_24h_change;
            metrics.funding_rate = rest_metrics.funding_rate;
            metrics.funding_rate_avg_7d = rest_metrics.funding_rate_avg_7d;
            metrics.funding_rate_std_7d = rest_metrics.funding_rate_std_7d;
            metrics.bid_volume_1pct = rest_metrics.bid_volume_1pct;
            metrics.ask_volume_1pct = rest_metrics.ask_volume_1pct;
            return Ok(metrics);
        }
        Err(e) => warn!("Bybit REST failed: {}", e),
    }
    
    // Fallback to Binance REST API
    match fetch_metrics_binance_rest(symbol).await {
        Ok(rest_metrics) => {
            metrics.current_price = rest_metrics.current_price;
            metrics.open_interest = rest_metrics.open_interest;
            metrics.open_interest_24h_change = rest_metrics.open_interest_24h_change;
            metrics.funding_rate = rest_metrics.funding_rate;
            metrics.funding_rate_avg_7d = rest_metrics.funding_rate_avg_7d;
            metrics.funding_rate_std_7d = rest_metrics.funding_rate_std_7d;
            metrics.bid_volume_1pct = rest_metrics.bid_volume_1pct;
            metrics.ask_volume_1pct = rest_metrics.ask_volume_1pct;
            return Ok(metrics);
        }
        Err(e) => warn!("Binance REST failed: {}", e),
    }
    
    // Final fallback: Use reasonable defaults for demonstration
    warn!("All exchange APIs failed, using default values for {}", symbol);
    setup_default_metrics(&mut metrics, symbol);
    
    Ok(metrics)
}

/// Fetch from Bybit v5 REST API
async fn fetch_metrics_bybit_rest(symbol: &str) -> Result<MarketMetrics, Box<dyn std::error::Error>> {
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0")
        .timeout(std::time::Duration::from_secs(10))
        .build()?;
    
    // Ticker endpoint
    let ticker_url = format!("https://api.bybit.com/v5/market/tickers?category=linear&symbol={}", symbol);
    let response = client.get(&ticker_url).send().await?;
    let json: Value = response.json().await?;
    
    let mut metrics = MarketMetrics::new(symbol);
    
    if let Some(result) = json.get("result").and_then(|r| r.get("list")).and_then(|l| l.get(0)) {
        // Parse last price
        if let Some(last_price) = result.get("lastPrice").and_then(|v| v.as_str()) {
            metrics.current_price = last_price.parse().unwrap_or(0.0);
        }
        
        // Parse open interest
        if let Some(open_interest) = result.get("openInterest").and_then(|v| v.as_str()) {
            metrics.open_interest = open_interest.parse().unwrap_or(0.0);
        }
        
        // Parse funding rate
        if let Some(funding_rate) = result.get("fundingRate").and_then(|v| v.as_str()) {
            metrics.funding_rate = funding_rate.parse().unwrap_or(0.0);
        }
    }
    
    if metrics.current_price == 0.0 {
        return Err("Bybit API returned zero price".into());
    }
    
    // Fetch additional data from Bybit funding rate endpoint
    let funding_url = format!("https://api.bybit.com/v5/market/funding/history?category=linear&symbol={}&limit=7", symbol);
    if let Ok(resp) = client.get(&funding_url).send().await {
        if let Ok(json) = resp.json::<Value>().await {
            if let Some(rates) = json.get("result").and_then(|r| r.get("list")) {
                let mut sum = 0.0;
                let mut count = 0;
                if let Some(list) = rates.as_array() {
                    for rate in list {
                        if let Some(r) = rate.get("fundingRate").and_then(|v| v.as_str()).and_then(|v| v.parse::<f64>().ok()) {
                            sum += r;
                            count += 1;
                        }
                    }
                }
                if count > 0 {
                    metrics.funding_rate_avg_7d = sum / count as f64;
                }
            }
        }
    }
    
    // Set realistic derivatives data
    metrics.open_interest_24h_change = 0.05; // +5% mock change
    metrics.funding_rate_std_7d = 0.00005;
    metrics.bid_volume_1pct = metrics.current_price * 50.0; // $5M bid depth mock
    metrics.ask_volume_1pct = metrics.current_price * 48.0; // $4.8M ask depth mock
    
    info!("Fetched {} metrics from Bybit REST: price=${:.2}", symbol, metrics.current_price);
    Ok(metrics)
}

/// Fetch from Binance Futures REST API
async fn fetch_metrics_binance_rest(symbol: &str) -> Result<MarketMetrics, Box<dyn std::error::Error>> {
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0")
        .timeout(std::time::Duration::from_secs(10))
        .build()?;
    
    // Binance ticker endpoint
    let ticker_url = format!("https://fapi.binance.com/fapi/v1/ticker/24hr?symbol={}", symbol);
    let response = client.get(&ticker_url).send().await?;
    let json: Value = response.json().await?;
    
    let mut metrics = MarketMetrics::new(symbol);
    
    // Parse last price
    if let Some(last_price) = json.get("lastPrice").and_then(|v| v.as_str()) {
        metrics.current_price = last_price.parse().unwrap_or(0.0);
    }
    
    // Parse open interest from separate endpoint
    let oi_url = format!("https://fapi.binance.com/fapi/v1/openInterest?symbol={}", symbol);
    if let Ok(resp) = client.get(&oi_url).send().await {
        if let Ok(json) = resp.json::<Value>().await {
            if let Some(oi) = json.get("openInterest").and_then(|v| v.as_str()) {
                metrics.open_interest = oi.parse().unwrap_or(0.0);
            }
        }
    }
    
    // Get funding rate
    let funding_url = format!("https://fapi.binance.com/fapi/v1/premiumIndex?symbol={}", symbol);
    if let Ok(resp) = client.get(&funding_url).send().await {
        if let Ok(json) = resp.json::<Value>().await {
            if let Some(rate) = json.get("lastFundingRate").and_then(|v| v.as_str()) {
                metrics.funding_rate = rate.parse().unwrap_or(0.0);
            }
        }
    }
    
    if metrics.current_price == 0.0 {
        return Err("Binance API returned zero price".into());
    }
    
    // Set realistic defaults
    metrics.open_interest_24h_change = 0.05;
    metrics.funding_rate_avg_7d = 0.0001;
    metrics.funding_rate_std_7d = 0.00005;
    metrics.bid_volume_1pct = metrics.current_price * 50.0;
    metrics.ask_volume_1pct = metrics.current_price * 48.0;
    
    info!("Fetched {} metrics from Binance REST: price=${:.2}", symbol, metrics.current_price);
    Ok(metrics)
}

/// Setup default metrics when APIs fail
fn setup_default_metrics(metrics: &mut MarketMetrics, symbol: &str) {
    let default_price = match symbol {
        "BTCUSDT" => 95000.0,
        "ETHUSDT" => 3500.0,
        "SOLUSDT" => 200.0,
        "AVAXUSDT" => 35.0,
        "LINKUSDT" => 20.0,
        _ => 100.0,
    };
    
    metrics.current_price = default_price;
    metrics.open_interest = default_price * 1000.0;
    metrics.open_interest_24h_change = 0.05;
    metrics.funding_rate = 0.0001;
    metrics.funding_rate_avg_7d = 0.0001;
    metrics.funding_rate_std_7d = 0.00005;
    metrics.bid_volume_1pct = 5000000.0;
    metrics.ask_volume_1pct = 4800000.0;
}

/// Maintain persistent WebSocket connection for real-time updates
pub async fn start_websocket_listener(symbol: &str) -> Result<(), Box<dyn std::error::Error>> {
    let ws_url = format!("{}", BYBIT_WS_URL);
    
    info!("Connecting to Bybit WebSocket: {}", ws_url);
    
    let (ws_stream, _) = connect_async(&ws_url).await?;
    let (mut write, mut read) = ws_stream.split();
    
    // Subscribe to ticker stream
    let subscribe_msg = serde_json::json!({
        "op": "subscribe",
        "args": [format!("tickers.{}", symbol)]
    });
    
    write.send(Message::Text(subscribe_msg.to_string())).await?;
    info!("Subscribed to tickers.{}", symbol);
    
    // Listen for messages
    while let Some(msg) = read.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                let json: Value = serde_json::from_str(&text)?;
                if let Some(data) = json.get("data").and_then(|d| d.get("list")).and_then(|l| l.get(0)) {
                    if let Some(price) = data.get("lastPrice").and_then(|v| v.as_str()) {
                        info!("{} price update: ${}", symbol, price);
                    }
                }
            }
            Ok(Message::Ping(data)) => {
                write.send(Message::Pong(data)).await?;
            }
            Err(e) => {
                error!("WebSocket error: {}", e);
                break;
            }
            _ => {}
        }
    }
    
    Ok(())
}

/// Calculate Funding Rate Deviation
/// F_dev = (F_current - F_avg_7d) / σ_F_7d
pub fn calculate_funding_deviation(
    current_rate: f64,
    avg_7d: f64,
    std_7d: f64,
) -> f64 {
    if std_7d == 0.0 {
        return 0.0;
    }
    (current_rate - avg_7d) / std_7d
}

/// Calculate Level-2 Liquidity Imbalance
/// LI = (ΣBid_Vol_1% - ΣAsk_Vol_1%) / (ΣBid_Vol_1% + ΣAsk_Vol_1%)
pub fn calculate_liquidity_imbalance(bid_vol: f64, ask_vol: f64) -> f64 {
    let total = bid_vol + ask_vol;
    if total == 0.0 {
        return 0.0;
    }
    (bid_vol - ask_vol) / total
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_funding_deviation() {
        let f_dev = calculate_funding_deviation(0.0002, 0.0001, 0.00005);
        assert!((f_dev - 2.0).abs() < 0.01);
    }

    #[test]
    fn test_liquidity_imbalance() {
        let li = calculate_liquidity_imbalance(5000000.0, 4800000.0);
        assert!(li > 0.0); // Positive means more bid pressure
    }
}
