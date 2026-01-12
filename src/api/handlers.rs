//! API Request Handlers

use alloy_primitives::{Address, U256};
use axum::{
    extract::{Json, State},
    http::StatusCode,
};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Semaphore;
use tracing::{info, error};

use super::types::*;
use crate::honeypot::HoneypotDetector;
use crate::risk_score::RiskScoreBuilder;
use crate::telemetry::TelemetryCollector;

/// Shared application state
pub struct AppState {
    pub telemetry: Arc<TelemetryCollector>,
    pub start_time: Instant,
    pub batch_semaphore: Arc<Semaphore>,
}

impl AppState {
    pub fn new(telemetry: Arc<TelemetryCollector>) -> Self {
        Self {
            telemetry,
            start_time: Instant::now(),
            batch_semaphore: Arc::new(Semaphore::new(100)), // Max 100 concurrent batch items
        }
    }

    pub fn uptime_seconds(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }
}

// ============================================
// Health Check
// ============================================

pub async fn health_check(State(state): State<Arc<AppState>>) -> Json<ApiResponse<HealthData>> {
    let start = Instant::now();

    let data = HealthData {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_seconds: state.uptime_seconds(),
    };

    Json(ApiResponse::success(
        data,
        start.elapsed().as_secs_f64() * 1000.0,
    ))
}

// ============================================
// Token Analysis
// ============================================

pub async fn analyze_token(
    State(state): State<Arc<AppState>>,
    Json(req): Json<TokenAnalysisRequest>,
) -> Result<Json<ApiResponse<TokenAnalysisData>>, (StatusCode, Json<ApiResponse<()>>)> {
    let start = Instant::now();

    // Validate address
    let token: Address = req.token_address.parse().map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error(
                ApiError::bad_request("Invalid token address format"),
                start.elapsed().as_secs_f64() * 1000.0,
            )),
        )
    })?;

    // Parse test amount
    let test_amount: f64 = req.test_amount_eth.parse().unwrap_or(0.1);
    let test_wei = U256::from((test_amount * 1e18) as u128);

    // Run honeypot detection (async with RPC)
    let detector = HoneypotDetector::mainnet();
    let hp_result = detector.detect_async(token, test_wei).await;

    // Build risk score
    let (risk_score, is_threat) = match hp_result {
        Ok(ref result) => {
            let score = RiskScoreBuilder::new()
                .with_honeypot_result(
                    result.is_honeypot,
                    result.sell_success,
                    result.total_loss_percent,
                )
                .with_tax_analysis(result.buy_tax_percent, result.sell_tax_percent)
                .build();
            (
                score,
                result.is_honeypot || result.total_loss_percent > 10.0,
            )
        }
        Err(_) => {
            // Simulation failed - return high risk
            let score = RiskScoreBuilder::new()
                .with_honeypot_result(false, false, 50.0)
                .build();
            (score, true)
        }
    };

    // Record telemetry - track threats properly
    let latency = start.elapsed().as_millis() as u64;
    if is_threat {
        use crate::telemetry::{TelemetryEvent, ThreatType};
        let event = TelemetryEvent::new(
            ThreatType::Honeypot,
            U256::from((test_amount * 1e18) as u128),
            latency,
            risk_score.total,
            format!("Token analysis: score {}", risk_score.total),
        );
        state.telemetry.record_threat(event);
    } else {
        state.telemetry.record_analysis(latency);
    }

    let data = TokenAnalysisData {
        token_address: req.token_address,
        risk_score: risk_score.into(),
        chain_id: req.chain_id,
    };

    Ok(Json(ApiResponse::success(
        data,
        start.elapsed().as_secs_f64() * 1000.0,
    )))
}

// ============================================
// Honeypot Check
// ============================================

pub async fn check_honeypot(
    State(state): State<Arc<AppState>>,
    Json(req): Json<HoneypotCheckRequest>,
) -> Result<Json<ApiResponse<HoneypotCheckData>>, (StatusCode, Json<ApiResponse<()>>)> {
    let start = Instant::now();

    // Validate address
    let token: Address = req.token_address.parse().map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error(
                ApiError::bad_request("Invalid token address format"),
                start.elapsed().as_secs_f64() * 1000.0,
            )),
        )
    })?;

    // Parse test amount
    let test_amount: f64 = req.test_amount_eth.parse().unwrap_or(0.1);
    let test_wei = U256::from((test_amount * 1e18) as u128);

    // Run detection with verbose logging
    info!("🔍 Starting REVM simulation for token: {}", req.token_address);
    info!("   Test amount: {} ETH ({} wei)", test_amount, test_wei);
    
    let detector = HoneypotDetector::mainnet();
    
    // Use async version to fetch real bytecode from RPC
    let result = detector.detect_async(token, test_wei).await;

    match &result {
        Ok(data) => {
            info!("✅ Simulation successful for {}", req.token_address);
            info!("   is_honeypot: {}, buy_success: {}, sell_success: {}", 
                  data.is_honeypot, data.buy_success, data.sell_success);
            info!("   buy_tax: {:.2}%, sell_tax: {:.2}%, total_loss: {:.2}%",
                  data.buy_tax_percent, data.sell_tax_percent, data.total_loss_percent);
            info!("   reason: {}", data.reason);
        }
        Err(e) => {
            error!("❌ SIMULATION FAILED for {}: {:?}", req.token_address, e);
        }
    }

    match result {
        Ok(hp_result) => {
            // Calculate risk score with PERS v2 algorithm:
            // - Sell reverted = 100 (confirmed honeypot)
            // - Is honeypot = 95
            // - High loss = 70
            // - Medium loss = 40
            // - Low loss = 10
            // + Access control penalty (0 or 50)
            let base_score = if hp_result.sell_reverted {
                100 // CONFIRMED HONEYPOT - sell reverted
            } else if hp_result.is_honeypot {
                95
            } else if hp_result.total_loss_percent > 30.0 {
                70
            } else if hp_result.total_loss_percent > 10.0 {
                40
            } else {
                10
            };

            // Add access control penalty (capped at 100)
            let risk_score = (base_score + hp_result.access_control_penalty as u32).min(100) as u8;

            // Record telemetry for honeypot checks
            let latency = start.elapsed().as_millis() as u64;
            if hp_result.is_honeypot || hp_result.sell_reverted {
                use crate::telemetry::{TelemetryEvent, ThreatType};
                let event = TelemetryEvent::new(
                    ThreatType::Honeypot,
                    U256::from((test_amount * 1e18) as u128),
                    latency,
                    risk_score,
                    hp_result.reason.clone(),
                );
                state.telemetry.record_threat(event);
            } else {
                state.telemetry.record_analysis(latency);
            }

            let data = HoneypotCheckData {
                token_address: req.token_address,
                is_honeypot: hp_result.is_honeypot || hp_result.sell_reverted,
                risk_score,
                buy_success: hp_result.buy_success,
                sell_success: hp_result.sell_success,
                buy_tax_percent: hp_result.buy_tax_percent,
                sell_tax_percent: hp_result.sell_tax_percent,
                total_loss_percent: hp_result.total_loss_percent,
                reason: hp_result.reason,
                simulation_latency_ms: hp_result.latency_ms,
            };

            Ok(Json(ApiResponse::success(
                data,
                start.elapsed().as_secs_f64() * 1000.0,
            )))
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::error(
                ApiError::internal(format!("Simulation failed: {}", e)),
                start.elapsed().as_secs_f64() * 1000.0,
            )),
        )),
    }
}

// ============================================
// Batch Analysis (NEW!)
// ============================================

pub async fn batch_analyze(
    State(state): State<Arc<AppState>>,
    Json(req): Json<BatchAnalysisRequest>,
) -> Result<Json<ApiResponse<BatchAnalysisData>>, (StatusCode, Json<ApiResponse<()>>)> {
    let start = Instant::now();

    // Validate request
    if req.tokens.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error(
                ApiError::bad_request("tokens array cannot be empty"),
                start.elapsed().as_secs_f64() * 1000.0,
            )),
        ));
    }

    if req.tokens.len() > 100 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error(
                ApiError::bad_request("Maximum 100 tokens per batch request"),
                start.elapsed().as_secs_f64() * 1000.0,
            )),
        ));
    }

    let concurrency = req.concurrency.clamp(1, 50);
    let test_amount: f64 = req.test_amount_eth.parse().unwrap_or(0.1);
    let test_wei = U256::from((test_amount * 1e18) as u128);

    // Process tokens concurrently
    let semaphore = Arc::new(Semaphore::new(concurrency));
    let mut handles = Vec::new();

    for token_addr in req.tokens.iter() {
        let sem = semaphore.clone();
        let addr = token_addr.clone();
        let wei = test_wei;

        let handle = tokio::spawn(async move {
            let _permit = sem.acquire().await.unwrap();
            let item_start = Instant::now();

            // Parse address
            let token: Result<Address, _> = addr.parse();

            match token {
                Ok(token) => {
                    let detector = HoneypotDetector::mainnet();
                    match detector.detect_async(token, wei).await {
                        Ok(result) => {
                            // PERS v2: sell_reverted = 100, + access_control_penalty
                            let base_score = if result.sell_reverted {
                                100
                            } else if result.is_honeypot {
                                95
                            } else if result.total_loss_percent > 30.0 {
                                70
                            } else if result.total_loss_percent > 10.0 {
                                40
                            } else {
                                10
                            };
                            let risk_score =
                                (base_score + result.access_control_penalty as u32).min(100) as u8;

                            let level = match risk_score {
                                0..=20 => "SAFE",
                                21..=40 => "LOW",
                                41..=60 => "MEDIUM",
                                61..=80 => "HIGH",
                                _ => "CRITICAL",
                            }
                            .to_string();

                            BatchTokenResult {
                                token_address: addr,
                                status: "success".to_string(),
                                risk_score: Some(risk_score),
                                is_honeypot: Some(result.is_honeypot),
                                level: Some(level),
                                error: None,
                                latency_ms: item_start.elapsed().as_secs_f64() * 1000.0,
                            }
                        }
                        Err(e) => BatchTokenResult {
                            token_address: addr,
                            status: "error".to_string(),
                            risk_score: None,
                            is_honeypot: None,
                            level: None,
                            error: Some(e.to_string()),
                            latency_ms: item_start.elapsed().as_secs_f64() * 1000.0,
                        },
                    }
                }
                Err(_) => BatchTokenResult {
                    token_address: addr,
                    status: "error".to_string(),
                    risk_score: None,
                    is_honeypot: None,
                    level: None,
                    error: Some("Invalid address format".to_string()),
                    latency_ms: item_start.elapsed().as_secs_f64() * 1000.0,
                },
            }
        });

        handles.push(handle);
    }

    // Collect results
    let mut results = Vec::new();
    for handle in handles {
        if let Ok(result) = handle.await {
            results.push(result);
        }
    }

    // Calculate summary
    let total_safe = results
        .iter()
        .filter(|r| r.risk_score.map(|s| s <= 40).unwrap_or(false))
        .count();
    let total_risky = results
        .iter()
        .filter(|r| r.risk_score.map(|s| s > 40).unwrap_or(false))
        .count();
    let total_honeypots = results
        .iter()
        .filter(|r| r.is_honeypot.unwrap_or(false))
        .count();

    // Record batch telemetry
    use crate::telemetry::{TelemetryEvent, ThreatType};
    for result in &results {
        if result.is_honeypot.unwrap_or(false) {
            let event = TelemetryEvent::new(
                ThreatType::Honeypot,
                U256::from((test_amount * 1e18) as u128),
                result.latency_ms as u64,
                result.risk_score.unwrap_or(95),
                format!("Batch: {}", result.token_address),
            );
            state.telemetry.record_threat(event);
        } else if result.status == "success" {
            state.telemetry.record_analysis(result.latency_ms as u64);
        }
    }

    let data = BatchAnalysisData {
        total_requested: req.tokens.len(),
        total_processed: results.len(),
        total_safe,
        total_risky,
        total_honeypots,
        results,
        processing_time_ms: start.elapsed().as_secs_f64() * 1000.0,
    };

    Ok(Json(ApiResponse::success(
        data,
        start.elapsed().as_secs_f64() * 1000.0,
    )))
}

// ============================================
// Stats
// ============================================

pub async fn get_stats(State(state): State<Arc<AppState>>) -> Json<ApiResponse<StatsData>> {
    let start = Instant::now();
    let stats = state.telemetry.get_stats();

    let data = StatsData {
        total_analyzed: stats.total_analyzed,
        total_threats: stats.total_threats,
        honeypots_detected: stats.honeypots_detected,
        total_value_protected_eth: stats.total_value_protected_eth,
        avg_latency_ms: stats.avg_latency_ms,
        uptime_seconds: state.uptime_seconds(),
        api_version: env!("CARGO_PKG_VERSION").to_string(),
    };

    Json(ApiResponse::success(
        data,
        start.elapsed().as_secs_f64() * 1000.0,
    ))
}
