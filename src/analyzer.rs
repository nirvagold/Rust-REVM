//! Core analyzer module
//! Orchestrates the entire transaction analysis pipeline

use alloy_primitives::{Address, B256, U256};
use alloy_provider::{Provider, ProviderBuilder, WsConnect};
use alloy_rpc_types::{Transaction, TransactionTrait};
use dashmap::DashMap;
use eyre::{Result, eyre};
use futures_util::StreamExt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use tokio::sync::Semaphore;
use tracing::{info, debug};

use crate::config::{DexRouters, SentryConfig};
use crate::decoder::SwapDecoder;
use crate::honeypot::HoneypotDetector;
use crate::telemetry::{TelemetryCollector, TelemetryEvent, ThreatType};
use crate::types::{AnalysisResult, RiskFactor, RiskLevel, SentryStats};

/// Main analyzer struct - the heart of Mempool Sentry
pub struct MempoolAnalyzer {
    /// Configuration
    config: SentryConfig,
    /// Known DEX routers
    dex_routers: DexRouters,
    /// Concurrency limiter
    semaphore: Arc<Semaphore>,
    /// Cache for recently seen transactions (avoid duplicates)
    seen_txs: Arc<DashMap<B256, ()>>,
    /// Statistics
    stats: Arc<AnalyzerStats>,
    /// Telemetry collector
    telemetry: Arc<TelemetryCollector>,
}

/// Thread-safe statistics
struct AnalyzerStats {
    total_received: AtomicU64,
    total_filtered: AtomicU64,
    total_analyzed: AtomicU64,
    total_risky: AtomicU64,
    total_latency_ms: AtomicU64,
}

impl Default for AnalyzerStats {
    fn default() -> Self {
        Self {
            total_received: AtomicU64::new(0),
            total_filtered: AtomicU64::new(0),
            total_analyzed: AtomicU64::new(0),
            total_risky: AtomicU64::new(0),
            total_latency_ms: AtomicU64::new(0),
        }
    }
}

impl MempoolAnalyzer {
    /// Create a new analyzer instance
    pub fn new(config: SentryConfig, telemetry: Arc<TelemetryCollector>) -> Self {
        let max_concurrent = config.max_concurrent_tasks;
        Self {
            config,
            dex_routers: DexRouters::default(),
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
            seen_txs: Arc::new(DashMap::new()),
            stats: Arc::new(AnalyzerStats::default()),
            telemetry,
        }
    }

    /// Start the mempool subscription and analysis loop
    pub async fn run(&self) -> Result<()> {
        info!("🚀 Starting Mempool Sentry...");
        info!("📡 Connecting to: {}", self.config.wss_url);

        // Connect via WebSocket
        let ws = WsConnect::new(&self.config.wss_url);
        let provider = ProviderBuilder::new()
            .on_ws(ws)
            .await
            .map_err(|e| eyre!("Failed to connect to WebSocket: {}", e))?;

        let provider = Arc::new(provider);
        
        info!("✅ Connected! Subscribing to pending transactions...");

        // Subscribe to pending transactions
        let sub = provider
            .subscribe_pending_transactions()
            .await
            .map_err(|e| eyre!("Failed to subscribe: {}", e))?;

        let mut stream = sub.into_stream();

        info!("🔍 Listening for mempool transactions...");
        self.print_stats_header();

        // Spawn stats printer
        let stats = self.stats.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(10));
            loop {
                interval.tick().await;
                let received = stats.total_received.load(Ordering::Relaxed);
                let filtered = stats.total_filtered.load(Ordering::Relaxed);
                let analyzed = stats.total_analyzed.load(Ordering::Relaxed);
                let risky = stats.total_risky.load(Ordering::Relaxed);
                let total_latency = stats.total_latency_ms.load(Ordering::Relaxed);
                let avg_latency = if analyzed > 0 { total_latency / analyzed } else { 0 };
                
                info!(
                    "📊 Stats | Received: {} | Filtered: {} | Analyzed: {} | Risky: {} | Avg Latency: {}ms",
                    received, filtered, analyzed, risky, avg_latency
                );
            }
        });

        // Process incoming transaction hashes
        while let Some(tx_hash) = stream.next().await {
            self.stats.total_received.fetch_add(1, Ordering::Relaxed);

            // Skip if already seen (dedup)
            if self.seen_txs.contains_key(&tx_hash) {
                continue;
            }
            self.seen_txs.insert(tx_hash, ());

            // Cleanup old entries periodically
            if self.seen_txs.len() > 10000 {
                self.seen_txs.clear();
            }

            // Spawn analysis task with concurrency limit
            let provider = provider.clone();
            let semaphore = self.semaphore.clone();
            let dex_routers = self.dex_routers.addresses.clone();
            let stats = self.stats.clone();
            let config = self.config.clone();
            let telemetry = self.telemetry.clone();

            tokio::spawn(async move {
                let _permit = match semaphore.acquire().await {
                    Ok(p) => p,
                    Err(_) => return,
                };

                // Fetch and process transaction
                match provider.get_transaction_by_hash(tx_hash).await {
                    Ok(Some(tx)) => {
                        if let Err(e) = process_transaction(
                            tx,
                            tx_hash,
                            &dex_routers,
                            stats,
                            &config,
                            telemetry,
                        ) {
                            debug!("Error processing tx {}: {}", tx_hash, e);
                        }
                    }
                    Ok(None) => {}
                    Err(e) => {
                        debug!("Error fetching tx {}: {}", tx_hash, e);
                    }
                }
            });
        }

        Ok(())
    }

    fn print_stats_header(&self) {
        println!("\n{}", "=".repeat(70));
        println!("  MEMPOOL SENTRY - Real-time Transaction Risk Analyzer");
        println!("  Monitoring DEX transactions for sandwich attacks & high slippage");
        println!("{}\n", "=".repeat(70));
    }

    /// Get current statistics
    pub fn get_stats(&self) -> SentryStats {
        let analyzed = self.stats.total_analyzed.load(Ordering::Relaxed);
        let total_latency = self.stats.total_latency_ms.load(Ordering::Relaxed);
        
        SentryStats {
            total_received: self.stats.total_received.load(Ordering::Relaxed),
            total_filtered: self.stats.total_filtered.load(Ordering::Relaxed),
            total_analyzed: analyzed,
            total_risky: self.stats.total_risky.load(Ordering::Relaxed),
            avg_latency_ms: if analyzed > 0 {
                total_latency as f64 / analyzed as f64
            } else {
                0.0
            },
        }
    }
}

/// Process a single transaction (synchronous analysis)
fn process_transaction(
    tx: Transaction,
    tx_hash: B256,
    dex_routers: &std::collections::HashSet<Address>,
    stats: Arc<AnalyzerStats>,
    config: &SentryConfig,
    telemetry: Arc<TelemetryCollector>,
) -> Result<()> {
    let start = Instant::now();

    // Extract fields
    let to_addr = TransactionTrait::to(&tx);
    let value = TransactionTrait::value(&tx);
    let gas_price = TransactionTrait::gas_price(&tx).unwrap_or(0);
    let input = TransactionTrait::input(&tx).clone();

    // CRITICAL FILTER: Only process DEX router transactions
    let target = match to_addr {
        Some(to) if dex_routers.contains(&to) => to,
        _ => {
            stats.total_filtered.fetch_add(1, Ordering::Relaxed);
            return Ok(());
        }
    };

    // Additional filter: Skip low gas price transactions (spam)
    let gas_price_gwei = gas_price / 1_000_000_000;
    if gas_price_gwei < config.min_gas_price_gwei as u128 {
        stats.total_filtered.fetch_add(1, Ordering::Relaxed);
        return Ok(());
    }

    stats.total_analyzed.fetch_add(1, Ordering::Relaxed);

    // Decode swap parameters
    let swap_params = SwapDecoder::decode(&input, value);

    // Create analysis result
    let mut result = AnalysisResult::new(
        tx_hash,
        tx.from,
        target,
        value,
        U256::from(gas_price),
    );

    // Analyze swap parameters for risks
    if let Some(ref params) = swap_params {
        // Check slippage tolerance
        if !params.amount_in.is_zero() && !params.amount_out_min.is_zero() {
            let ratio = params.amount_out_min
                .saturating_mul(U256::from(10000))
                .checked_div(params.amount_in)
                .unwrap_or(U256::from(10000));
            
            let ratio_u64: u64 = ratio.try_into().unwrap_or(10000);
            if ratio_u64 < 10000 - config.slippage_threshold_bps {
                let slippage_bps = 10000 - ratio_u64;
                result.add_risk(RiskFactor::HighSlippage {
                    expected_bps: 100,
                    actual_bps: slippage_bps,
                });
            }
        }

        // Check for sandwich attack vulnerability
        let value_eth = wei_to_eth(params.amount_in);
        if value_eth > 0.5 {
            let slippage_pct = if !params.amount_in.is_zero() {
                let ratio = params.amount_out_min
                    .saturating_mul(U256::from(100))
                    .checked_div(params.amount_in)
                    .unwrap_or(U256::from(100));
                100u64.saturating_sub(ratio.try_into().unwrap_or(100))
            } else {
                0
            };

            if slippage_pct > 3 {
                result.add_risk(RiskFactor::SandwichTarget {
                    reason: format!(
                        "Swap of {:.4} ETH with {}% slippage - prime MEV target",
                        value_eth, slippage_pct
                    ),
                });
            }
        }

        // Check for fee-on-transfer tokens
        if input.len() >= 4 {
            let selector = &input[..4];
            if selector == [0x79, 0x1a, 0xc9, 0x47] ||
               selector == [0xb6, 0xf9, 0xde, 0x95] ||
               selector == [0x5c, 0x11, 0xd7, 0x95] {
                result.add_risk(RiskFactor::HighTax {
                    tax_bps: 500,
                });
            }
        }

        // ============================================
        // HONEYPOT DETECTION via REVM Simulation
        // Only for swaps > 0.1 ETH (worth the compute)
        // ============================================
        if value_eth > 0.1 && params.path.len() >= 2 {
            let token_address = params.path.last().cloned();
            
            if let Some(token) = token_address {
                // Skip WETH (not a token swap)
                let weth: Address = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"
                    .parse()
                    .unwrap_or_default();
                
                if token != weth {
                    let detector = HoneypotDetector::mainnet();
                    let test_amount = U256::from(100_000_000_000_000_000u128); // 0.1 ETH
                    
                    match detector.detect(token, test_amount, None, None, None, None) {
                        Ok(hp_result) => {
                            if hp_result.is_honeypot {
                                result.add_risk(RiskFactor::Honeypot {
                                    reason: hp_result.reason,
                                    buy_success: hp_result.buy_success,
                                    sell_success: hp_result.sell_success,
                                });
                            } else if hp_result.total_loss_percent > 10.0 {
                                // High tax but not honeypot
                                result.add_risk(RiskFactor::HighRoundTripTax {
                                    buy_tax: hp_result.buy_tax_percent,
                                    sell_tax: hp_result.sell_tax_percent,
                                    total_loss: hp_result.total_loss_percent,
                                });
                            }
                        }
                        Err(_) => {
                            // Simulation failed - could be honeypot
                            result.add_risk(RiskFactor::SimulationFailed {
                                reason: "Honeypot check failed - proceed with caution".to_string(),
                            });
                        }
                    }
                }
            }
        }
    }

    // Check for unusual gas price (potential front-run)
    if gas_price_gwei > 100 {
        result.add_risk(RiskFactor::UnusualGasPrice {
            gas_gwei: gas_price_gwei as u64,
            avg_gwei: 30,
        });
    }

    // Check for large value
    let value_eth = wei_to_eth(value);
    if value_eth > 10.0 {
        result.add_risk(RiskFactor::LargeValue { value_eth });
    }

    // Set latency
    result.set_latency(start);
    stats.total_latency_ms.fetch_add(result.latency_ms, Ordering::Relaxed);

    // Update risky count
    if result.risk_level as u8 >= RiskLevel::Medium as u8 {
        stats.total_risky.fetch_add(1, Ordering::Relaxed);
    }

    // ============================================
    // TELEMETRY RECORDING
    // ============================================
    if !result.risk_factors.is_empty() {
        // Record each threat type
        for factor in &result.risk_factors {
            let threat_type = match factor {
                RiskFactor::Honeypot { .. } => ThreatType::Honeypot,
                RiskFactor::HighSlippage { .. } => ThreatType::HighSlippage,
                RiskFactor::SandwichTarget { .. } => ThreatType::SandwichTarget,
                RiskFactor::HighTax { .. } => ThreatType::HighTax,
                RiskFactor::HighRoundTripTax { .. } => ThreatType::HighTax,
                RiskFactor::UnusualGasPrice { .. } => ThreatType::UnusualGas,
                RiskFactor::LargeValue { .. } => ThreatType::LargeValue,
                RiskFactor::SimulationFailed { .. } => ThreatType::SimulationFailed,
                RiskFactor::UnverifiedContract => continue, // Skip this one
            };
            
            let event = TelemetryEvent::new(
                threat_type,
                value,
                result.latency_ms,
                result.risk_level as u8,
                factor.description(),
            );
            
            telemetry.record_threat(event);
        }
    } else {
        // Record clean analysis
        telemetry.record_analysis(result.latency_ms);
    }

    // Output result for risky transactions
    if result.risk_level as u8 >= RiskLevel::Low as u8 {
        println!("{}", result.summary());
    }

    Ok(())
}

/// Convert wei to ETH
fn wei_to_eth(wei: U256) -> f64 {
    let wei_u128: u128 = wei.try_into().unwrap_or(u128::MAX);
    wei_u128 as f64 / 1e18
}

// Make SentryConfig cloneable for async tasks
impl Clone for SentryConfig {
    fn clone(&self) -> Self {
        Self {
            wss_url: self.wss_url.clone(),
            http_url: self.http_url.clone(),
            max_concurrent_tasks: self.max_concurrent_tasks,
            rpc_timeout: self.rpc_timeout,
            min_gas_price_gwei: self.min_gas_price_gwei,
            slippage_threshold_bps: self.slippage_threshold_bps,
            high_tax_threshold_bps: self.high_tax_threshold_bps,
        }
    }
}
