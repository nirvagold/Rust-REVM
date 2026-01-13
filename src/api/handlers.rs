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
use crate::utils::constants::{is_solana_address, CHAIN_ID_SOLANA};
use crate::providers::dexscreener::DexScreenerClient;
use crate::providers::solana::SolanaClient;
use crate::core::honeypot::HoneypotDetector;
use crate::core::risk_score::RiskScoreBuilder;
use crate::core::ml_risk::{MLRiskScorer, MLFeatureSet, LiquidityFeatures, TradingFeatures, SocialFeatures};
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

    // ============================================
    // SOLANA DETECTION - Check if address is Solana format
    // ============================================
    if is_solana_address(&req.token_address) || req.chain_id == CHAIN_ID_SOLANA {
        info!("🌐 Detected Solana token: {}", req.token_address);
        return handle_solana_token(&state, &req, start).await;
    }

    // Validate EVM address
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
                    token_name: best.base_token.name.clone(),
                    token_symbol: best.base_token.symbol.clone(),
                    all_pairs: vec![],
                    has_v2_liquidity: !v3_only,
                    total_pairs: pairs.len(),
                    // Market data
                    price_usd: best.price_usd.clone(),
                    volume_24h_usd: best.volume.as_ref().and_then(|v| v.h24),
                    pair_address: Some(best.pair_address.clone()),
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

        // Extract market data from detected_info
        let (price_usd, liquidity_usd, volume_24h_usd, pair_address) = match &detected_info {
            Some(info) => (
                info.price_usd.clone(),
                Some(info.best_dex.liquidity_usd),
                info.volume_24h_usd,
                info.pair_address.clone(),
            ),
            None => (None, None, None, None),
        };
        
        let data = HoneypotCheckData {
            token_address: req.token_address,
            token_name: auto_detected_name,
            token_symbol: auto_detected_symbol,
            token_decimals: None,
            chain_id: effective_chain_id,
            chain_name: chain_name.clone(),
            native_symbol: "ETH".to_string(),
            is_honeypot: false,
            risk_score: 70, // HIGH risk - cannot verify
            buy_success: false,
            sell_success: false,
            buy_tax_percent: 0.0,
            sell_tax_percent: 0.0,
            total_loss_percent: 0.0,
            reason: format!("Token only available on {} (V3/Velodrome-style) - not supported yet. Use DEX directly.", dex_name),
            simulation_latency_ms: start.elapsed().as_millis() as u64,
            // DexScreener market data
            price_usd,
            liquidity_usd,
            volume_24h_usd,
            dex_name: Some(dex_name),
            pair_address,
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
        
        // ALWAYS fetch token name/symbol from RPC (instant, no DexScreener delay)
        let token_info = detector.fetch_token_info(token).await;
        let (token_name, token_symbol, token_decimals) = (token_info.name, token_info.symbol, token_info.decimals);

        // Market data from DexScreener (optional, with timeout)
        let (price_usd, liquidity_usd, volume_24h_usd, dex_name, pair_address) = 
            fetch_market_data_optional(&req.token_address, chain_id).await;
        
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
            // DexScreener market data
            price_usd,
            liquidity_usd,
            volume_24h_usd,
            dex_name,
            pair_address,
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

            // ALWAYS fetch token name/symbol from RPC (instant, no DexScreener delay)
            let token_info = detector.fetch_token_info(token).await;
            let (token_name, token_symbol, token_decimals) = (token_info.name, token_info.symbol, token_info.decimals);
            info!("📛 Token info from RPC: {:?} ({:?})", token_name, token_symbol);

            // Market data from DexScreener (optional, with timeout)
            let (price_usd, liquidity_usd, volume_24h_usd, dex_name, pair_address) = 
                fetch_market_data_optional(&req.token_address, chain_id).await;

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
                // DexScreener market data
                price_usd,
                liquidity_usd,
                volume_24h_usd,
                dex_name,
                pair_address,
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
    // Special case: No liquidity found - THIS IS SUSPICIOUS!
    // If we can't simulate buy/sell, we can't verify safety
    // Treat as HIGH RISK (not safe to trade)
    if !result.buy_success && !result.sell_success && !result.is_honeypot && !result.sell_reverted {
        // No liquidity = UNVERIFIED = HIGH RISK
        // User should NOT trade tokens we can't verify
        return 70; // "HIGH" risk - cannot verify safety
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

// ============================================
// MARKET DATA HELPER (DexScreener with timeout)
// ============================================

/// Fetch market data from DexScreener with 3 second timeout
/// Returns (price_usd, liquidity_usd, volume_24h_usd, dex_name, pair_address)
async fn fetch_market_data_optional(
    token_address: &str,
    chain_id: u64,
) -> (Option<String>, Option<f64>, Option<f64>, Option<String>, Option<String>) {
    let dexscreener = DexScreenerClient::new();
    
    match tokio::time::timeout(
        std::time::Duration::from_secs(3),
        dexscreener.get_pairs_for_chain(token_address, chain_id)
    ).await {
        Ok(Ok(pairs)) if !pairs.is_empty() => {
            let best = &pairs[0];
            (
                best.price_usd.clone(),
                best.liquidity.as_ref().and_then(|l| l.usd),
                best.volume.as_ref().and_then(|v| v.h24),
                Some(best.dex_id.clone()),
                Some(best.pair_address.clone()),
            )
        }
        _ => (None, None, None, None, None)
    }
}

// ============================================
// SOLANA TOKEN HANDLER
// ============================================

/// Handle Solana token analysis using DexScreener + Solana RPC
async fn handle_solana_token(
    state: &Arc<AppState>,
    req: &HoneypotCheckRequest,
    start: Instant,
) -> Result<Json<ApiResponse<HoneypotCheckData>>, (StatusCode, Json<ApiResponse<()>>)> {
    info!("🌐 Analyzing Solana token: {}", req.token_address);
    
    // Get DexScreener data first
    let dexscreener = DexScreenerClient::new();
    let dex_data = dexscreener.get_token_pairs(&req.token_address).await.ok();
    
    // Filter for Solana pairs
    let solana_pairs: Vec<_> = dex_data
        .as_ref()
        .map(|pairs| pairs.iter().filter(|p| p.chain_id.to_lowercase() == "solana").collect())
        .unwrap_or_default();
    
    if solana_pairs.is_empty() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ApiResponse::error(
                ApiError::not_found("Token not found on Solana DEXes"),
                start.elapsed().as_secs_f64() * 1000.0,
            )),
        ));
    }
    
    // Get best pair (highest liquidity)
    let best_pair = solana_pairs.iter()
        .max_by(|a, b| {
            let liq_a = a.liquidity.as_ref().and_then(|l| l.usd).unwrap_or(0.0);
            let liq_b = b.liquidity.as_ref().and_then(|l| l.usd).unwrap_or(0.0);
            liq_a.partial_cmp(&liq_b).unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap();
    
    // Extract market data
    let token_name = best_pair.base_token.name.clone();
    let token_symbol = best_pair.base_token.symbol.clone();
    let price_usd = best_pair.price_usd.clone();
    let liquidity_usd = best_pair.liquidity.as_ref().and_then(|l| l.usd);
    let volume_24h_usd = best_pair.volume.as_ref().and_then(|v| v.h24);
    let dex_name = Some(best_pair.dex_id.clone());
    let pair_address = Some(best_pair.pair_address.clone());
    
    // Try to get additional data from Solana RPC
    let solana_analysis = match SolanaClient::new() {
        Ok(client) => client.analyze_token(&req.token_address).await.ok(),
        Err(e) => {
            warn!("⚠️ Solana RPC not available: {}", e);
            None
        }
    };
    
    // Calculate risk score using ML scorer
    let features = MLFeatureSet {
        liquidity: LiquidityFeatures {
            total_liquidity_usd: liquidity_usd.unwrap_or(0.0),
            is_locked: false,
            lock_duration_days: 0,
            lp_holder_count: 0,
            top_lp_holder_percent: 0.0,
            pool_count: solana_pairs.len() as u32,
        },
        trading: TradingFeatures {
            volume_24h_usd: volume_24h_usd.unwrap_or(0.0),
            holder_count: 0,
            top_10_holder_percent: 0.0,
            buy_count_24h: 0,
            sell_count_24h: 0,
            largest_sell_percent: 0.0,
            price_change_24h: 0.0,
        },
        social: SocialFeatures {
            age_hours: 1,
            has_website: false,
            has_twitter: false,
            has_telegram: false,
            twitter_followers: 0,
            telegram_members: 0,
            is_verified_project: false,
        },
        ..Default::default()
    };
    
    // Check if pump.fun
    let is_pump_fun = best_pair.dex_id.to_lowercase().contains("pump");
    
    // Calculate ML risk score
    let ml_scorer = MLRiskScorer::new();
    let ml_result = ml_scorer.calculate_score(&features);
    
    // Adjust risk for pump.fun tokens (inherently risky)
    let mut risk_score = ml_result.score as u8;
    if is_pump_fun {
        risk_score = risk_score.saturating_add(20).min(100);
    }
    
    // Low liquidity penalty
    if liquidity_usd.unwrap_or(0.0) < 10000.0 {
        risk_score = risk_score.saturating_add(15).min(100);
    }
    
    // Determine if honeypot based on Solana analysis
    let is_honeypot = solana_analysis.as_ref().map(|a| a.is_honeypot).unwrap_or(false);
    if is_honeypot {
        risk_score = risk_score.max(80);
    }
    
    // Build reason string
    let mut reasons = Vec::new();
    if is_pump_fun {
        reasons.push("Pump.fun token (high risk category)");
    }
    if liquidity_usd.unwrap_or(0.0) < 10000.0 {
        reasons.push("Low liquidity");
    }
    if let Some(ref analysis) = solana_analysis {
        for flag in &analysis.red_flags {
            reasons.push(flag.as_str());
        }
    }
    
    let reason = if reasons.is_empty() {
        "Solana token analyzed via DexScreener".to_string()
    } else {
        format!("Solana token: {}", reasons.join(", "))
    };
    
    // Record telemetry
    state.telemetry.record_analysis(start.elapsed().as_millis() as u64);
    
    // Clone for logging before moving into struct
    let symbol_for_log = token_symbol.clone();
    
    let data = HoneypotCheckData {
        token_address: req.token_address.clone(),
        token_name,
        token_symbol,
        token_decimals: solana_analysis.as_ref().map(|_| 6), // Most Solana tokens use 6 decimals
        chain_id: CHAIN_ID_SOLANA,
        chain_name: "Solana".to_string(),
        native_symbol: "SOL".to_string(),
        is_honeypot,
        risk_score,
        buy_success: true, // Can't simulate on Solana
        sell_success: !is_honeypot,
        buy_tax_percent: 0.0, // Solana doesn't have built-in tax
        sell_tax_percent: 0.0,
        total_loss_percent: 0.0,
        reason,
        simulation_latency_ms: start.elapsed().as_millis() as u64,
        price_usd,
        liquidity_usd,
        volume_24h_usd,
        dex_name,
        pair_address,
    };
    
    info!("✅ Solana analysis complete: {} - Risk: {}/100", 
          symbol_for_log.as_deref().unwrap_or("Unknown"), risk_score);
    
    Ok(Json(ApiResponse::success(
        data,
        start.elapsed().as_secs_f64() * 1000.0,
    )))
}
