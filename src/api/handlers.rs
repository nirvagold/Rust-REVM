//! API Request Handlers

use alloy_primitives::{Address, U256};
use axum::{
    extract::{Json, State},
    http::StatusCode,
};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Semaphore;
use tracing::{info, error, warn};

use super::types::*;
use crate::utils::cache::HoneypotCache;
use crate::providers::dexscreener::DexScreenerClient;
use crate::core::honeypot::HoneypotDetector;
use crate::core::risk_score::RiskScoreBuilder;
use crate::utils::telemetry::TelemetryCollector;

/// Shared application state
pub struct AppState {
    pub telemetry: Arc<TelemetryCollector>,
    pub cache: Arc<HoneypotCache>,
    pub start_time: Instant,
    pub batch_semaphore: Arc<Semaphore>,
}

impl AppState {
    pub fn new(telemetry: Arc<TelemetryCollector>) -> Self {
        let cache = Arc::new(HoneypotCache::new());
        
        // Background task: cleanup expired cache entries every 60 seconds
        let cache_clone = cache.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
            loop {
                interval.tick().await;
                let removed = cache_clone.cleanup_expired();
                if removed > 0 {
                    tracing::info!("🧹 Cache cleanup: {} expired entries removed", removed);
                }
            }
        });

        Self {
            telemetry,
            cache,
            start_time: Instant::now(),
            batch_semaphore: Arc::new(Semaphore::new(100)),
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
        use crate::utils::telemetry::{TelemetryEvent, ThreatType};
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

    // ============================================
    // AUTO-DETECT CHAIN & DEX via DexScreener
    // This finds the actual DEX with liquidity
    // ============================================
    let dexscreener = DexScreenerClient::new();
    
    let (effective_chain_id, detected_info, is_v3_only) = if req.chain_id == 0 {
        info!("🔍 Auto-detecting chain & DEX for {}...", req.token_address);
        
        match dexscreener.auto_detect_token(&req.token_address).await {
            Ok(detected) => {
                info!("✅ Auto-detected: {} ({}) on {} via {}", 
                      detected.token_symbol.as_deref().unwrap_or("Unknown"),
                      detected.token_name.as_deref().unwrap_or("Unknown"),
                      detected.chain_name,
                      detected.best_dex.dex_name);
                info!("   Liquidity: ${:.2}, Router: {:?}, V2: {}", 
                      detected.best_dex.liquidity_usd,
                      detected.best_dex.router_address,
                      detected.has_v2_liquidity);
                let v3_only = !detected.has_v2_liquidity && detected.total_pairs > 0;
                (detected.chain_id, Some(detected), v3_only)
            }
            Err(e) => {
                warn!("⚠️ DexScreener failed: {}. Token may not be listed.", e);
                (1, None, false) // Default to Ethereum, no DEX info
            }
        }
    } else {
        // Chain specified, but still try to get DEX info from DexScreener
        info!("🔍 Looking up DEX info for {} on chain {}...", req.token_address, req.chain_id);
        match dexscreener.get_pairs_for_chain(&req.token_address, req.chain_id).await {
            Ok(pairs) if !pairs.is_empty() => {
                // Check if any V2 pairs exist
                let v2_pairs: Vec<_> = pairs.iter().filter(|p| p.is_v2_compatible()).collect();
                let best = if !v2_pairs.is_empty() {
                    v2_pairs[0].clone()
                } else {
                    pairs[0].clone()
                };
                let discovered = best.to_discovered_dex();
                let v3_only = v2_pairs.is_empty();
                info!("✅ Found on {} with ${:.2} liquidity (V3-only: {})", 
                      discovered.dex_name, discovered.liquidity_usd, v3_only);
                
                (req.chain_id, Some(crate::providers::dexscreener::AutoDetectedToken {
                    chain_id: req.chain_id,
                    chain_name: crate::providers::dexscreener::DexScreenerClient::chain_id_to_name_pub(req.chain_id).to_string(),
                    best_dex: discovered,
                    token_name: best.base_token.name,
                    token_symbol: best.base_token.symbol,
                    all_pairs: vec![],
                    has_v2_liquidity: !v3_only,
                    total_pairs: pairs.len(),
                }), v3_only)
            }
            _ => {
                info!("📭 No DexScreener data for chain {}", req.chain_id);
                (req.chain_id, None, false)
            }
        }
    };

    // Extract info from DexScreener result
    let (auto_detected_name, auto_detected_symbol, discovered_router) = match &detected_info {
        Some(info) => (
            info.token_name.clone(),
            info.token_symbol.clone(),
            info.best_dex.router_address.clone(),
        ),
        None => (None, None, None),
    };

    // If token is V3-only, return early with appropriate message
    if is_v3_only {
        let chain_name = detected_info.as_ref()
            .map(|i| i.chain_name.clone())
            .unwrap_or_else(|| "Unknown".to_string());
        let dex_name = detected_info.as_ref()
            .map(|i| i.best_dex.dex_name.clone())
            .unwrap_or_else(|| "Unknown DEX".to_string());
        
        info!("⚠️ Token only available on V3/Velodrome-style DEX: {}", dex_name);
        
        let data = HoneypotCheckData {
            token_address: req.token_address,
            token_name: auto_detected_name,
            token_symbol: auto_detected_symbol,
            token_decimals: None,
            chain_id: effective_chain_id,
            chain_name,
            native_symbol: "ETH".to_string(),
            is_honeypot: false,
            risk_score: 0,
            buy_success: false,
            sell_success: false,
            buy_tax_percent: 0.0,
            sell_tax_percent: 0.0,
            total_loss_percent: 0.0,
            reason: format!("Token only available on {} (V3/Velodrome-style) - not supported yet. Use DEX directly.", dex_name),
            simulation_latency_ms: start.elapsed().as_millis() as u64,
        };

        return Ok(Json(ApiResponse::success(
            data,
            start.elapsed().as_secs_f64() * 1000.0,
        )));
    }

    // Get detector for detected/specified chain
    let detector = HoneypotDetector::for_chain(effective_chain_id).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error(
                ApiError::bad_request(format!(
                    "Unsupported chain_id: {}. Supported: 1 (ETH), 56 (BSC), 137 (Polygon), 42161 (Arbitrum), 10 (Optimism), 43114 (Avalanche), 8453 (Base)",
                    effective_chain_id
                )),
                start.elapsed().as_secs_f64() * 1000.0,
            )),
        )
    })?;

    let chain_id = detector.chain_id;
    let chain_name = detector.chain_name.clone();
    let native_symbol = detector.native_symbol.clone();

    info!("🔗 Chain: {} ({}) - {}", chain_name, chain_id, native_symbol);

    // Cache key includes chain_id for multi-chain support
    let cache_key = format!("{}:{}", chain_id, req.token_address.to_lowercase());

    // ============================================
    // CACHE-FIRST: Check cache before RPC call
    // ============================================
    if let Some(cached_result) = state.cache.get(&cache_key) {
        info!("⚡ Returning cached result for {} on {}", req.token_address, chain_name);
        
        // Use auto-detected name/symbol if available, otherwise fetch from RPC
        let (token_name, token_symbol, token_decimals) = if auto_detected_name.is_some() {
            (auto_detected_name, auto_detected_symbol, None)
        } else {
            let token_info = detector.fetch_token_info(token).await;
            (token_info.name, token_info.symbol, token_info.decimals)
        };
        
        // Calculate risk score from cached result
        let risk_score = calculate_risk_score(&cached_result);
        
        let data = HoneypotCheckData {
            token_address: req.token_address,
            token_name,
            token_symbol,
            token_decimals,
            chain_id,
            chain_name,
            native_symbol,
            is_honeypot: cached_result.is_honeypot || cached_result.sell_reverted,
            risk_score,
            buy_success: cached_result.buy_success,
            sell_success: cached_result.sell_success,
            buy_tax_percent: cached_result.buy_tax_percent,
            sell_tax_percent: cached_result.sell_tax_percent,
            total_loss_percent: cached_result.total_loss_percent,
            reason: format!("{} (cached)", cached_result.reason),
            simulation_latency_ms: 0, // Instant from cache
        };

        return Ok(Json(ApiResponse::success(
            data,
            start.elapsed().as_secs_f64() * 1000.0,
        )));
    }

    // ============================================
    // CACHE MISS: Perform RPC simulation
    // ============================================
    let test_amount: f64 = req.test_amount_eth.parse().unwrap_or(0.1);
    let test_wei = U256::from((test_amount * 1e18) as u128);

    info!("🔍 CACHE MISS - Starting RPC simulation for: {} on {}", req.token_address, chain_name);
    info!("   Test amount: {} {}", test_amount, native_symbol);
    
    // If DexScreener found a router, add it as priority
    let detector = if let Some(router_addr) = discovered_router {
        if let Ok(router) = router_addr.parse::<alloy_primitives::Address>() {
            let dex_name = detected_info.as_ref()
                .map(|i| i.best_dex.dex_name.clone())
                .unwrap_or_else(|| "DexScreener".to_string());
            info!("🎯 Using DexScreener router: {} ({})", dex_name, router_addr);
            detector.with_priority_router(dex_name, router)
        } else {
            detector
        }
    } else {
        detector
    };
    
    let result = detector.detect_async(token, test_wei).await;

    match &result {
        Ok(data) => {
            info!("✅ Simulation successful for {} on {}", req.token_address, chain_name);
            info!("   is_honeypot: {}, buy_success: {}, sell_success: {}", 
                  data.is_honeypot, data.buy_success, data.sell_success);
            info!("   buy_tax: {:.2}%, sell_tax: {:.2}%, total_loss: {:.2}%",
                  data.buy_tax_percent, data.sell_tax_percent, data.total_loss_percent);
        }
        Err(e) => {
            error!("❌ SIMULATION FAILED for {}: {:?}", req.token_address, e);
            // DO NOT cache failed results per CEO directive
        }
    }

    match result {
        Ok(hp_result) => {
            // ============================================
            // CACHE SET: Store valid result (with chain_id in key)
            // ============================================
            state.cache.set(&cache_key, hp_result.clone());

            // Use auto-detected name/symbol if available, otherwise fetch from RPC
            let (token_name, token_symbol, token_decimals) = if auto_detected_name.is_some() {
                (auto_detected_name, auto_detected_symbol, None)
            } else {
                let token_info = detector.fetch_token_info(token).await;
                info!("📛 Token info: {:?}", token_info);
                (token_info.name, token_info.symbol, token_info.decimals)
            };

            // Calculate risk score based on actual simulation results
            let risk_score = calculate_risk_score(&hp_result);

            // Record telemetry for honeypot checks
            let latency = start.elapsed().as_millis() as u64;
            if hp_result.is_honeypot || hp_result.sell_reverted {
                use crate::utils::telemetry::{TelemetryEvent, ThreatType};
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
                token_name,
                token_symbol,
                token_decimals,
                chain_id,
                chain_name,
                native_symbol,
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
    use crate::utils::telemetry::{TelemetryEvent, ThreatType};
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
    let cache_stats = state.cache.stats();

    let data = StatsData {
        total_analyzed: stats.total_analyzed,
        total_threats: stats.total_threats,
        honeypots_detected: stats.honeypots_detected,
        total_value_protected_eth: stats.total_value_protected_eth,
        avg_latency_ms: stats.avg_latency_ms,
        uptime_seconds: state.uptime_seconds(),
        api_version: env!("CARGO_PKG_VERSION").to_string(),
    };

    // Log cache stats for CEO monitoring
    info!("📊 Cache Stats: {} entries, {:.1}% hit rate ({} hits / {} misses)",
          cache_stats.entries, cache_stats.hit_rate, cache_stats.hits, cache_stats.misses);

    Json(ApiResponse::success(
        data,
        start.elapsed().as_secs_f64() * 1000.0,
    ))
}

// ============================================
// Helper Functions
// ============================================

/// Calculate risk score from HoneypotResult
/// PERS v2 algorithm implementation
fn calculate_risk_score(result: &crate::core::honeypot::HoneypotResult) -> u8 {
    // Special case: No liquidity found - not necessarily dangerous
    // Token might just trade on a different DEX
    if !result.buy_success && !result.sell_success && !result.is_honeypot && !result.sell_reverted {
        // No liquidity = unknown, give neutral score
        return 30; // "LOW" risk - we just couldn't test it
    }

    // Base score based on simulation results
    let base_score = if result.sell_reverted {
        100 // CONFIRMED HONEYPOT - sell reverted
    } else if result.is_honeypot {
        95
    } else if result.total_loss_percent > 50.0 {
        80 // Extreme tax
    } else if result.total_loss_percent > 30.0 {
        60 // High tax
    } else if result.total_loss_percent > 10.0 {
        40 // Medium tax
    } else if result.total_loss_percent > 5.0 {
        20 // Low tax
    } else {
        10 // Safe - minimal loss
    };

    // Only add access control penalty if there's suspicious loss
    let penalty = if result.total_loss_percent > 5.0 {
        result.access_control_penalty as u32
    } else {
        0 // Ignore for low-loss tokens (likely legit)
    };

    // Cap at 100
    (base_score + penalty).min(100) as u8
}
