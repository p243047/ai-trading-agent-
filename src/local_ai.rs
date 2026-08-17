//! Local AI Integration Module
//! 
//! Connects to Ollama local LLM for narrative analysis and risk verification

use reqwest;
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error};
use crate::types::{AIVerification, MarketMetrics};

/// Default Ollama API endpoint
const OLLAMA_BASE_URL: &str = "http://localhost:11434";

/// Default model to use (can be configured)
const DEFAULT_MODEL: &str = "llama3.2:3b";

/// Request structure for Ollama API
#[derive(Debug, Serialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
    stream: bool,
    format: String,
}

/// Response structure from Ollama API
#[derive(Debug, Deserialize)]
struct OllamaResponse {
    response: String,
    done: bool,
}

/// Review trade context using local Ollama LLM
/// Sends market metrics and receives AI verification with confidence adjustment
pub async fn review_trade_context(
    symbol: &str,
    probability: f64,
    metrics: &MarketMetrics,
) -> Result<AIVerification, Box<dyn std::error::Error>> {
    // Try to connect to Ollama, but don't fail if unavailable
    match review_with_ollama(symbol, probability, metrics).await {
        Ok(verification) => Ok(verification),
        Err(e) => {
            warn!("Ollama not available or error occurred: {}. Using default verification.", e);
            Ok(AIVerification::default())
        }
    }
}

/// Internal function to communicate with Ollama API
async fn review_with_ollama(
    symbol: &str,
    probability: f64,
    metrics: &MarketMetrics,
) -> Result<AIVerification, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    
    // Build the prompt for AI analysis
    let prompt = format!(
        r#"You are an expert quantitative risk manager.
Evaluate the following asset data:
- Symbol: {}
- Calculated Probability: {:.2}%
- BlackRock Flow Score: ${:.2}M
- OI Change 24h: {:.2}%
- Funding Rate: {:.6}
- Current Price: ${:.2}

Task: Verify if there are any obvious narrative risks or macro anomalies.
Output JSON format: {{"approved": true/false, "confidence_adjustment": +0.05/-0.05, "reason": "summary"}}

Respond ONLY with valid JSON."#,
        symbol,
        probability * 100.0,
        metrics.blackrock_flow_usd / 1_000_000.0,
        metrics.open_interest_24h_change * 100.0,
        metrics.funding_rate,
        metrics.current_price
    );
    
    let request = OllamaRequest {
        model: DEFAULT_MODEL.to_string(),
        prompt,
        stream: false,
        format: "json".to_string(),
    };
    
    let url = format!("{}/api/generate", OLLAMA_BASE_URL);
    
    let response = client
        .post(&url)
        .json(&request)
        .send()
        .await?;
    
    if !response.status().is_success() {
        return Err(format!("Ollama API returned status: {}", response.status()).into());
    }
    
    let ollama_response: OllamaResponse = response.json().await?;
    
    // Parse the JSON response from the LLM
    parse_ai_response(&ollama_response.response)
}

/// Parse AI response into AIVerification struct
fn parse_ai_response(response_text: &str) -> Result<AIVerification, Box<dyn std::error::Error>> {
    // Try to extract JSON from the response
    let json_start = response_text.find('{').unwrap_or(0);
    let json_end = response_text.rfind('}').unwrap_or(response_text.len() - 1);
    let json_str = &response_text[json_start..=json_end];
    
    #[derive(Debug, Deserialize)]
    struct AIResponseJson {
        approved: bool,
        confidence_adjustment: f64,
        reason: String,
    }
    
    let parsed: AIResponseJson = serde_json::from_str(json_str)?;
    
    // Clamp confidence adjustment to reasonable bounds
    let adjustment = parsed.confidence_adjustment.clamp(-0.10, 0.10);
    
    Ok(AIVerification {
        approved: parsed.approved,
        confidence_adjustment: adjustment,
        reason: parsed.reason,
    })
}

/// Alternative: Simple heuristic-based verification when Ollama is unavailable
pub fn heuristic_verification(probability: f64, metrics: &MarketMetrics) -> AIVerification {
    let mut approved = true;
    let mut adjustment = 0.0;
    let mut reasons = Vec::new();
    
    // Check for extreme funding rates (overcrowded positions)
    if metrics.funding_rate > 0.001 {
        approved = false;
        adjustment -= 0.05;
        reasons.push("Extreme positive funding rate detected");
    }
    
    // Check for very low probability trades
    if probability < 0.50 || probability > 0.70 {
        adjustment -= 0.03;
        reasons.push("Probability outside optimal range");
    }
    
    // Check for large OI changes (potential manipulation)
    if metrics.open_interest_24h_change.abs() > 0.20 {
        approved = false;
        adjustment -= 0.05;
        reasons.push("Unusual open interest spike");
    }
    
    AIVerification {
        approved,
        confidence_adjustment: adjustment,
        reason: if reasons.is_empty() {
            "No significant risks detected".to_string()
        } else {
            reasons.join("; ")
        },
    }
}

/// Test connection to Ollama
pub async fn test_ollama_connection() -> bool {
    let client = reqwest::Client::new();
    let url = format!("{}/api/tags", OLLAMA_BASE_URL);
    
    match client.get(&url).send().await {
        Ok(response) => response.status().is_success(),
        Err(_) => false,
    }
}

/// List available models on Ollama
pub async fn list_available_models() -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let url = format!("{}/api/tags", OLLAMA_BASE_URL);
    
    let response = client.get(&url).send().await?;
    let json: serde_json::Value = response.json().await?;
    
    let models: Vec<String> = json
        .get("models")
        .and_then(|m| m.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|model| model.get("name").and_then(|n| n.as_str()))
                .map(|s| s.to_string())
                .collect()
        })
        .unwrap_or_default();
    
    Ok(models)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heuristic_verification_extreme_funding() {
        let mut metrics = MarketMetrics::new("BTCUSDT");
        metrics.funding_rate = 0.002; // Extreme
        
        let verification = heuristic_verification(0.60, &metrics);
        assert!(!verification.approved);
        assert!(verification.confidence_adjustment < 0.0);
    }

    #[test]
    fn test_heuristic_verification_normal() {
        let metrics = MarketMetrics::new("BTCUSDT");
        
        let verification = heuristic_verification(0.60, &metrics);
        assert!(verification.approved);
    }
}
