use reqwest;
use crate::types::{AiVerification, MarketMetrics};
use tracing::{info, warn};

/// Review trade context using local Ollama LLM
pub async fn review_trade_context(
    symbol: &str,
    probability: f64,
    metrics: &MarketMetrics,
) -> Result<AiVerification, Box<dyn std::error::Error>> {
    // Try Ollama first, fallback to heuristic if unavailable
    match call_ollama(symbol, probability, metrics).await {
        Ok(verification) => {
            info!("AI verification for {}: approved={}, reason={}", 
                symbol, verification.approved, verification.reason);
            Ok(verification)
        }
        Err(e) => {
            warn!("Ollama unavailable ({}), using heuristic verification", e);
            Ok(heuristic_verification(symbol, probability, metrics))
        }
    }
}

async fn call_ollama(
    symbol: &str,
    probability: f64,
    metrics: &MarketMetrics,
) -> Result<AiVerification, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    
    let prompt = format!(
        r#"You are an expert quantitative risk manager.
Evaluate the following asset data:
- Symbol: {}
- Calculated Probability: {:.2}%
- BlackRock Flow Score: ${:.2}M
- OI Change 24h: {:.2}%
- Funding Rate: {:.6}
- Price Change 24h: {:.2}%

Task: Verify if there are any obvious narrative risks or macro anomalies.
Output ONLY valid JSON in this exact format:
{{"approved": true/false, "confidence_adjustment": 0.05/-0.05/0.0, "reason": "brief summary"}}

Rules:
- If probability > 70% and funding rate is extreme (>0.001), reduce confidence
- If BlackRock flows are strongly positive (>100M), increase confidence  
- If OI surging while price dropping, flag liquidation risk
- Keep reason under 50 words"#,
        symbol,
        probability * 100.0,
        metrics.blackrock_flow_usd / 1_000_000.0,
        metrics.oi_change_24h_pct,
        metrics.funding_rate,
        metrics.price_change_24h_pct,
    );
    
    let payload = serde_json::json!({
        "model": "llama3.2:3b",
        "prompt": prompt,
        "stream": false,
        "format": "json"
    });
    
    let response = client
        .post("http://localhost:11434/api/generate")
        .json(&payload)
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await?;
    
    if !response.status().is_success() {
        return Err("Ollama request failed".into());
    }
    
    let json: serde_json::Value = response.json().await?;
    
    let response_text = json
        .get("response")
        .and_then(|r| r.as_str())
        .ok_or("No response field in Ollama output")?;
    
    // Parse the JSON from response
    let verification: AiVerification = serde_json::from_str(response_text)?;
    
    Ok(verification)
}

/// Heuristic verification when Ollama is unavailable
fn heuristic_verification(
    _symbol: &str,
    probability: f64,
    metrics: &MarketMetrics,
) -> AiVerification {
    let mut approved = true;
    let mut adjustment: f64 = 0.0;
    let mut reasons = Vec::new();
    
    // Check for extreme funding rates
    if metrics.funding_rate > 0.001 {
        reasons.push("Extreme positive funding - overcrowded longs");
        adjustment -= 0.05;
    } else if metrics.funding_rate < -0.001 {
        reasons.push("Extreme negative funding - overcrowded shorts");
        adjustment += 0.03;
    }
    
    // Check BlackRock flows
    let flow_millions = metrics.blackrock_flow_usd / 1_000_000.0;
    if flow_millions > 200.0 {
        reasons.push("Strong institutional inflow detected");
        adjustment += 0.05;
    } else if flow_millions < -100.0 {
        reasons.push("Institutional outflow warning");
        adjustment -= 0.03;
    }
    
    // Check for OI/Price divergence (liquidation signal)
    if metrics.price_change_24h_pct < -2.0 && metrics.oi_change_24h_pct < -3.0 {
        reasons.push("Long liquidation squeeze in progress");
        adjustment -= 0.10;
        if probability < 0.40 {
            approved = false;
        }
    }
    
    // High probability check
    if probability > 0.75 && metrics.funding_rate > 0.0005 {
        reasons.push("High probability but crowded trade");
        adjustment -= 0.05;
    }
    
    let reason = if reasons.is_empty() {
        "No significant anomalies detected. Trade parameters within normal range.".to_string()
    } else {
        reasons.join(". ")
    };
    
    AiVerification {
        approved,
        confidence_adjustment: adjustment.clamp(-0.15, 0.15),
        reason,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::MarketMetrics;
    use chrono::Utc;
    
    fn create_test_metrics() -> MarketMetrics {
        MarketMetrics {
            symbol: "BTCUSDT".to_string(),
            current_price: 95000.0,
            funding_rate: 0.0001,
            open_interest: 5000000000.0,
            oi_change_24h_pct: 2.5,
            price_change_24h_pct: 1.8,
            bid_volume_1pct: 750000000.0,
            ask_volume_1pct: 600000000.0,
            blackrock_flow_usd: 150_000_000.0,
            timestamp: Utc::now(),
        }
    }
    
    #[test]
    fn test_heuristic_normal_conditions() {
        let metrics = create_test_metrics();
        let result = heuristic_verification("BTCUSDT", 0.65, &metrics);
        
        assert!(result.approved);
        println!("Heuristic result: approved={}, adj={:.2}, reason={}", 
            result.approved, result.confidence_adjustment, result.reason);
    }
    
    #[test]
    fn test_heuristic_extreme_funding() {
        let mut metrics = create_test_metrics();
        metrics.funding_rate = 0.0015; // Extreme
        
        let result = heuristic_verification("BTCUSDT", 0.70, &metrics);
        
        assert!(result.confidence_adjustment < 0.0);
    }
    
    #[test]
    fn test_heuristic_strong_inflow() {
        let mut metrics = create_test_metrics();
        metrics.blackrock_flow_usd = 300_000_000.0; // Strong inflow
        
        let result = heuristic_verification("BTCUSDT", 0.60, &metrics);
        
        assert!(result.confidence_adjustment > 0.0);
    }
    
    #[test]
    fn test_heuristic_liquidation_scenario() {
        let mut metrics = create_test_metrics();
        metrics.price_change_24h_pct = -4.0;
        metrics.oi_change_24h_pct = -5.0;
        
        let result = heuristic_verification("BTCUSDT", 0.35, &metrics);
        
        assert!(result.reason.contains("liquidation"));
    }
}
