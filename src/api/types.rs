//! API Request/Response Types

use crate::risk_score::RiskScore;
use serde::{Deserialize, Serialize};

/// API Response wrapper
#[derive(Debug, Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ApiError>,
    pub latency_ms: f64,
    pub timestamp: i64,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn success(data: T, latency_ms: f64) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            latency_ms,
            timestamp: chrono::Utc::now().timestamp(),
        }
    }
}

impl ApiResponse<()> {
    pub fn error(error: ApiError, latency_ms: f64) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(error),
            latency_ms,
            timestamp: chrono::Utc::now().timestamp(),
        }
    }
}

/// API Error
#[derive(Debug, Serialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

impl ApiError {
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self {
            code: "BAD_REQUEST".to_string(),
            message: message.into(),
            details: None,
        }
    }

    pub fn unauthorized() -> Self {
        Self {
            code: "UNAUTHORIZED".to_string(),
            message: "Invalid or missing API key".to_string(),
            details: None,
        }
    }

    pub fn rate_limited(retry_after: u64) -> Self {
        Self {
            code: "RATE_LIMITED".to_string(),
            message: format!("Rate limit exceeded. Retry after {} seconds", retry_after),
            details: Some(format!("retry_after: {}", retry_after)),
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            code: "INTERNAL_ERROR".to_string(),
            message: message.into(),
            details: None,
        }
    }
}

// ============================================
// Token Analysis
// ============================================

#[derive(Debug, Deserialize)]
pub struct TokenAnalysisRequest {
    pub token_address: String,
    #[serde(default = "default_test_amount")]
    pub test_amount_eth: String,
    #[serde(default = "default_chain_id")]
    pub chain_id: u64,
}

fn default_test_amount() -> String {
    "0.1".to_string()
}
fn default_chain_id() -> u64 {
    1
}

#[derive(Debug, Serialize)]
pub struct TokenAnalysisData {
    pub token_address: String,
    pub risk_score: RiskScoreResponse,
    pub chain_id: u64,
}

#[derive(Debug, Serialize)]
pub struct RiskScoreResponse {
    pub total: u8,
    pub confidence: u8,
    pub recommendation: String,
    pub is_gray_area: bool,
    pub level: String,
    pub color: String,
    pub components: RiskComponentsResponse,
    pub breakdown: Vec<ScoreFactorResponse>,
}

#[derive(Debug, Serialize)]
pub struct RiskComponentsResponse {
    pub honeypot: u8,
    pub tax: u8,
    pub liquidity: u8,
    pub contract: u8,
    pub mev_exposure: u8,
}

#[derive(Debug, Serialize)]
pub struct ScoreFactorResponse {
    pub name: String,
    pub score: u8,
    pub weight: f32,
    pub reason: String,
}

impl From<RiskScore> for RiskScoreResponse {
    fn from(score: RiskScore) -> Self {
        Self {
            total: score.total,
            confidence: score.confidence,
            recommendation: score.recommendation.clone(),
            is_gray_area: score.is_gray_area(),
            level: match score.total {
                0..=20 => "SAFE",
                21..=40 => "LOW",
                41..=60 => "MEDIUM",
                61..=80 => "HIGH",
                _ => "CRITICAL",
            }
            .to_string(),
            color: score.color_code().to_string(),
            components: RiskComponentsResponse {
                honeypot: score.components.honeypot,
                tax: score.components.tax,
                liquidity: score.components.liquidity,
                contract: score.components.contract,
                mev_exposure: score.components.mev_exposure,
            },
            breakdown: score
                .breakdown
                .into_iter()
                .map(|f| ScoreFactorResponse {
                    name: f.name,
                    score: f.score,
                    weight: f.weight,
                    reason: f.reason,
                })
                .collect(),
        }
    }
}

// ============================================
// Honeypot Check
// ============================================

#[derive(Debug, Deserialize)]
pub struct HoneypotCheckRequest {
    pub token_address: String,
    #[serde(default = "default_test_amount")]
    pub test_amount_eth: String,
    /// Chain ID (1 = Ethereum, 56 = BSC, 137 = Polygon, etc.)
    /// Default: 1 (Ethereum)
    #[serde(default = "default_chain_id")]
    pub chain_id: u64,
}

#[derive(Debug, Serialize)]
pub struct HoneypotCheckData {
    pub token_address: String,
    /// Token name (e.g., "Tether USD")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_name: Option<String>,
    /// Token symbol (e.g., "USDT")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_symbol: Option<String>,
    /// Token decimals (e.g., 18)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_decimals: Option<u8>,
    /// Chain ID (1 = Ethereum, 56 = BSC, etc.)
    pub chain_id: u64,
    /// Chain name (e.g., "Ethereum", "BNB Smart Chain")
    pub chain_name: String,
    /// Native token symbol (e.g., "ETH", "BNB")
    pub native_symbol: String,
    pub is_honeypot: bool,
    pub risk_score: u8,
    pub buy_success: bool,
    pub sell_success: bool,
    pub buy_tax_percent: f64,
    pub sell_tax_percent: f64,
    pub total_loss_percent: f64,
    pub reason: String,
    pub simulation_latency_ms: u64,
}

// ============================================
// Batch Analysis (NEW!)
// ============================================

#[derive(Debug, Deserialize)]
pub struct BatchAnalysisRequest {
    pub tokens: Vec<String>,
    #[serde(default = "default_test_amount")]
    pub test_amount_eth: String,
    #[serde(default = "default_chain_id")]
    pub chain_id: u64,
    /// Max concurrent checks (default: 10, max: 50)
    #[serde(default = "default_concurrency")]
    pub concurrency: usize,
}

fn default_concurrency() -> usize {
    10
}

#[derive(Debug, Serialize)]
pub struct BatchAnalysisData {
    pub total_requested: usize,
    pub total_processed: usize,
    pub total_safe: usize,
    pub total_risky: usize,
    pub total_honeypots: usize,
    pub results: Vec<BatchTokenResult>,
    pub processing_time_ms: f64,
}

#[derive(Debug, Serialize)]
pub struct BatchTokenResult {
    pub token_address: String,
    pub status: String, // "success" | "error"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub risk_score: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_honeypot: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub latency_ms: f64,
}

// ============================================
// Stats / Telemetry
// ============================================

#[derive(Debug, Serialize)]
pub struct StatsData {
    pub total_analyzed: u64,
    pub total_threats: u64,
    pub honeypots_detected: u64,
    pub total_value_protected_eth: f64,
    pub avg_latency_ms: f64,
    pub uptime_seconds: u64,
    pub api_version: String,
}

// ============================================
// Health Check
// ============================================

#[derive(Debug, Serialize)]
pub struct HealthData {
    pub status: String,
    pub version: String,
    pub uptime_seconds: u64,
}
