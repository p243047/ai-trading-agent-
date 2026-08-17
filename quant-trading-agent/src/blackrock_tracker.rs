use reqwest;
use scraper::{Html, Selector};
use tracing::{info, warn};
use crate::types::MarketMetrics;

/// Fetch BlackRock ETF flows from multiple sources
pub async fn fetch_latest_etf_flows() -> Result<f64, Box<dyn std::error::Error>> {
    // Try Farside Investors first
    if let Ok(flow) = fetch_from_farside().await {
        info!("BlackRock IBIT flow from Farside: ${:.2}M", flow);
        return Ok(flow * 1_000_000.0); // Convert to USD
    }
    
    // Fallback to Coinglass API
    if let Ok(flow) = fetch_from_coinglass().await {
        info!("BlackRock IBIT flow from Coinglass: ${:.2}M", flow);
        return Ok(flow * 1_000_000.0);
    }
    
    // Default to zero if all sources fail
    warn!("All ETF flow sources failed, using default value");
    Ok(0.0)
}

async fn fetch_from_farside() -> Result<f64, Box<dyn std::error::Error>> {
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .timeout(std::time::Duration::from_secs(10))
        .build()?;
    
    let response = client
        .get("https://farside.co.uk/?p=3890")
        .send()
        .await?;
    
    if !response.status().is_success() {
        return Err("Farside request failed".into());
    }
    
    let html = response.text().await?;
    let document = Html::parse_document(&html);
    
    // Look for IBIT row in table
    let selector = Selector::parse("td").unwrap();
    let mut prev_td = String::new();
    
    for td in document.select(&selector) {
        let text = td.text().collect::<String>().trim().to_string();
        if text.contains("IBIT") || prev_td.contains("BlackRock") {
            // Try to parse the next cell as flow value
            if let Ok(val) = text.replace("$", "").replace("M", "").replace(",", "").parse::<f64>() {
                return Ok(val);
            }
        }
        prev_td = text;
    }
    
    Err("No IBIT data found".into())
}

async fn fetch_from_coinglass() -> Result<f64, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    
    // Coinglass public API for ETF flows
    let response = client
        .get("https://open-api.coinglass.com/api/v1/etf/flow")
        .header("accept", "application/json")
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await?;
    
    if !response.status().is_success() {
        return Err("Coinglass request failed".into());
    }
    
    let json: serde_json::Value = response.json().await?;
    
    // Parse IBIT flow from response
    if let Some(data) = json.get("data").and_then(|d| d.as_array()) {
        for item in data {
            if let Some(name) = item.get("name").and_then(|n| n.as_str()) {
                if name.contains("IBIT") || name.contains("BlackRock") {
                    if let Some(flow) = item.get("inflow").and_then(|f| f.as_f64()) {
                        return Ok(flow);
                    }
                }
            }
        }
    }
    
    Err("No IBIT data in Coinglass".into())
}

/// Calculate Z-score for BlackRock flows
pub fn calculate_br_zscore(current_flow: f64, historical_flows: &[f64]) -> f64 {
    if historical_flows.is_empty() {
        return 0.0;
    }
    
    let mean: f64 = historical_flows.iter().sum::<f64>() / historical_flows.len() as f64;
    let variance: f64 = historical_flows
        .iter()
        .map(|x| (x - mean).powi(2))
        .sum::<f64>() / historical_flows.len() as f64;
    let std_dev = variance.sqrt();
    
    if std_dev == 0.0 {
        return 0.0;
    }
    
    (current_flow - mean) / std_dev
}

/// Simulate historical flows for demonstration (in production, store in DB)
pub fn get_historical_flows() -> Vec<f64> {
    vec![
        150.0, 200.0, -50.0, 300.0, 100.0, -100.0, 250.0, 
        180.0, -30.0, 400.0, 120.0, -80.0, 350.0, 90.0,
        -120.0, 280.0, 160.0, -60.0, 320.0, 140.0, -90.0,
        270.0, 190.0, -40.0, 380.0, 110.0, -70.0, 290.0,
        170.0, -50.0,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_zscore_calculation() {
        let flows = vec![100.0, 200.0, 150.0, 180.0, 170.0];
        let zscore = calculate_br_zscore(300.0, &flows);
        assert!(zscore > 1.0, "Z-score should be high for outlier");
    }
    
    #[test]
    fn test_historical_flows_not_empty() {
        let flows = get_historical_flows();
        assert!(!flows.is_empty());
        assert_eq!(flows.len(), 30);
    }
}
