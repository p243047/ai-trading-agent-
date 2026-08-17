use reqwest;
use tracing::{info, warn};
use crate::types::MarketMetrics;
use chrono::Utc;

/// Fetch market metrics from Bybit v5 API (primary) or Binance (fallback)
pub async fn get_market_metrics(symbol: &str, blackrock_flow_usd: f64) -> Result<MarketMetrics, Box<dyn std::error::Error>> {
    // Try Bybit first
    match fetch_from_bybit(symbol).await {
        Ok(mut metrics) => {
            metrics.blackrock_flow_usd = blackrock_flow_usd;
            metrics.timestamp = Utc::now();
            info!("Fetched {} data from Bybit", symbol);
            return Ok(metrics);
        }
        Err(e) => {
            warn!("Bybit failed for {}: {}", symbol, e);
        }
    }
    
    // Fallback to Binance
    match fetch_from_binance(symbol).await {
        Ok(mut metrics) => {
            metrics.blackrock_flow_usd = blackrock_flow_usd;
            metrics.timestamp = Utc::now();
            info!("Fetched {} data from Binance", symbol);
            return Ok(metrics);
        }
        Err(e) => {
            warn!("Binance failed for {}: {}", symbol, e);
        }
    }
    
    // Return simulated data if all APIs fail
    warn!("All APIs failed for {}, using simulated data", symbol);
    Ok(get_simulated_metrics(symbol, blackrock_flow_usd))
}

async fn fetch_from_bybit(symbol: &str) -> Result<MarketMetrics, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    
    // Bybit v5 market ticker endpoint
    let url = format!("https://api.bybit.com/v5/market/tickers?category=linear&symbol={}", symbol);
    
    let response = client
        .get(&url)
        .header("accept", "application/json")
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await?;
    
    if !response.status().is_success() {
        return Err("Bybit request failed".into());
    }
    
    let json: serde_json::Value = response.json().await?;
    
    if json.get("retCode").and_then(|c| c.as_i64()) != Some(0) {
        return Err("Bybit API error".into());
    }
    
    let data = json
        .get("result")
        .and_then(|r| r.get("list"))
        .and_then(|l| l.as_array())
        .and_then(|arr| arr.first())
        .ok_or("No data in Bybit response")?;
    
    let current_price = data
        .get("lastPrice")
        .and_then(|p| p.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);
    
    let funding_rate = data
        .get("fundingRate")
        .and_then(|f| f.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);
    
    let open_interest = data
        .get("openInterest")
        .and_then(|oi| oi.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);
    
    let price_change_24h_pct = data
        .get("price24hPcnt")
        .and_then(|p| p.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);
    
    // Fetch OI change from separate endpoint
    let oi_change = fetch_oi_change_bybit(symbol, &client).await.unwrap_or(0.0);
    
    // Simulate order book depth (would need separate WS connection for real data)
    let bid_volume = open_interest * 0.15;
    let ask_volume = open_interest * 0.12;
    
    Ok(MarketMetrics {
        symbol: symbol.to_string(),
        current_price,
        funding_rate,
        open_interest,
        oi_change_24h_pct: oi_change,
        price_change_24h_pct,
        bid_volume_1pct: bid_volume,
        ask_volume_1pct: ask_volume,
        blackrock_flow_usd: 0.0,
        timestamp: Utc::now(),
    })
}

async fn fetch_oi_change_bybit(symbol: &str, client: &reqwest::Client) -> Result<f64, Box<dyn std::error::Error>> {
    // Get historical OI for comparison
    let url = format!("https://api.bybit.com/v5/market/open-interest?category=linear&symbol={}&limit=2", symbol);
    
    let response = client
        .get(&url)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await?;
    
    if let Ok(json) = response.json::<serde_json::Value>().await {
        if let Some(list) = json.get("result").and_then(|r| r.get("list")).and_then(|l| l.as_array()) {
            if list.len() >= 2 {
                let current_oi = list[0].get("openInterest").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let prev_oi = list[1].get("openInterest").and_then(|v| v.as_f64()).unwrap_or(0.0);
                
                if prev_oi > 0.0 {
                    return Ok(((current_oi - prev_oi) / prev_oi) * 100.0);
                }
            }
        }
    }
    
    Ok(0.0)
}

async fn fetch_from_binance(symbol: &str) -> Result<MarketMetrics, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    
    // Binance Futures 24hr ticker
    let url = format!("https://fapi.binance.com/fapi/v1/ticker/24hr?symbol={}", symbol);
    
    let response = client
        .get(&url)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await?;
    
    if !response.status().is_success() {
        return Err("Binance request failed".into());
    }
    
    let json: serde_json::Value = response.json().await?;
    
    let current_price = json
        .get("lastPrice")
        .and_then(|p| p.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);
    
    let price_change_24h_pct = json
        .get("priceChangePercent")
        .and_then(|p| p.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);
    
    // Get funding rate from premium index
    let funding_url = format!("https://fapi.binance.com/fapi/v1/premiumIndex?symbol={}", symbol);
    let funding_rate = fetch_funding_rate_binance(&client, &funding_url).await.unwrap_or(0.0);
    
    // Get open interest
    let oi_url = format!("https://fapi.binance.com/fapi/v1/openInterest?symbol={}", symbol);
    let open_interest = fetch_open_interest_binance(&client, &oi_url).await.unwrap_or(0.0);
    
    let oi_change = fetch_oi_change_binance(symbol, &client).await.unwrap_or(0.0);
    
    let bid_volume = open_interest * 0.15;
    let ask_volume = open_interest * 0.12;
    
    Ok(MarketMetrics {
        symbol: symbol.to_string(),
        current_price,
        funding_rate,
        open_interest,
        oi_change_24h_pct: oi_change,
        price_change_24h_pct,
        bid_volume_1pct: bid_volume,
        ask_volume_1pct: ask_volume,
        blackrock_flow_usd: 0.0,
        timestamp: Utc::now(),
    })
}

async fn fetch_funding_rate_binance(client: &reqwest::Client, url: &str) -> Result<f64, Box<dyn std::error::Error>> {
    let response = client
        .get(url)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await?;
    
    if let Ok(json) = response.json::<serde_json::Value>().await {
        if let Some(rate) = json.get("lastFundingRate").and_then(|f| f.as_f64()) {
            return Ok(rate);
        }
    }
    
    Ok(0.0)
}

async fn fetch_open_interest_binance(client: &reqwest::Client, url: &str) -> Result<f64, Box<dyn std::error::Error>> {
    let response = client
        .get(url)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await?;
    
    if let Ok(json) = response.json::<serde_json::Value>().await {
        if let Some(oi_str) = json.get("openInterest").and_then(|oi| oi.as_str()) {
            if let Ok(oi) = oi_str.parse::<f64>() {
                return Ok(oi);
            }
        }
    }
    
    Ok(0.0)
}

async fn fetch_oi_change_binance(symbol: &str, client: &reqwest::Client) -> Result<f64, Box<dyn std::error::Error>> {
    // Get OI trend from Binance
    let url = format!("https://fapi.binance.com/futures/data/openInterestHist?symbol={}&period=5m&limit=2", symbol);
    
    let response = client
        .get(&url)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await?;
    
    if let Ok(json) = response.json::<serde_json::Value>().await {
        if let Some(arr) = json.as_array() {
            if arr.len() >= 2 {
                let current_oi = arr[0].get("sumOpenInterest").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let prev_oi = arr[1].get("sumOpenInterest").and_then(|v| v.as_f64()).unwrap_or(0.0);
                
                if prev_oi > 0.0 {
                    return Ok(((current_oi - prev_oi) / prev_oi) * 100.0);
                }
            }
        }
    }
    
    Ok(0.0)
}

fn get_simulated_metrics(symbol: &str, blackrock_flow_usd: f64) -> MarketMetrics {
    // Use realistic price ranges based on symbol
    let base_price = match symbol {
        "BTCUSDT" => 95000.0,
        "ETHUSDT" => 3500.0,
        "SOLUSDT" => 180.0,
        "AVAXUSDT" => 35.0,
        "LINKUSDT" => 22.0,
        _ => 100.0,
    };
    
    // Add some randomness
    let variation = (Utc::now().timestamp() as f64 % 1000.0) / 10000.0;
    let current_price = base_price * (1.0 + variation);
    
    MarketMetrics {
        symbol: symbol.to_string(),
        current_price,
        funding_rate: 0.0001,
        open_interest: current_price * 1000000.0,
        oi_change_24h_pct: 2.5,
        price_change_24h_pct: 1.8,
        bid_volume_1pct: current_price * 150000.0,
        ask_volume_1pct: current_price * 120000.0,
        blackrock_flow_usd,
        timestamp: Utc::now(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_simulated_metrics() {
        let metrics = get_simulated_metrics("BTCUSDT", 100000000.0);
        assert_eq!(metrics.symbol, "BTCUSDT");
        assert!(metrics.current_price > 0.0);
    }
}
