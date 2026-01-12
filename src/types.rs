//! Type definitions for Mempool Sentry
//! All core data structures for transaction analysis

use alloy_primitives::{Address, B256, U256};
use serde::{Deserialize, Serialize};
use std::time::Instant;

/// Risk level classification for transactions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskLevel {
    /// Transaction appears safe
    Safe,
    /// Low risk - minor concerns
    Low,
    /// Medium risk - proceed with caution
    Medium,
    /// High risk - likely to lose funds
    High,
    /// Critical - almost certain loss (sandwich target, honeypot, etc.)
    Critical,
}

impl RiskLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            RiskLevel::Safe => "SAFE",
            RiskLevel::Low => "LOW",
            RiskLevel::Medium => "MEDIUM",
            RiskLevel::High => "HIGH",
            RiskLevel::Critical => "CRITICAL",
        }
    }

    pub fn emoji(&self) -> &'static str {
        match self {
            RiskLevel::Safe => "✅",
            RiskLevel::Low => "🟡",
            RiskLevel::Medium => "🟠",
            RiskLevel::High => "🔴",
            RiskLevel::Critical => "💀",
        }
    }
}

/// Detected risk factors in a transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RiskFactor {
    /// High slippage detected
    HighSlippage { expected_bps: u64, actual_bps: u64 },
    /// Token has high transfer tax
    HighTax { tax_bps: u64 },
    /// Potential sandwich attack target
    SandwichTarget { reason: String },
    /// Honeypot token detected (CRITICAL - cannot sell)
    Honeypot {
        reason: String,
        buy_success: bool,
        sell_success: bool,
    },
    /// Unusual gas price (front-run indicator)
    UnusualGasPrice { gas_gwei: u64, avg_gwei: u64 },
    /// Large value transaction (whale alert)
    LargeValue { value_eth: f64 },
    /// Contract interaction with unverified code
    UnverifiedContract,
    /// Simulation failed
    SimulationFailed { reason: String },
    /// High round-trip tax detected via simulation
    HighRoundTripTax {
        buy_tax: f64,
        sell_tax: f64,
        total_loss: f64,
    },
}

impl RiskFactor {
    pub fn description(&self) -> String {
        match self {
            RiskFactor::HighSlippage {
                expected_bps,
                actual_bps,
            } => {
                format!(
                    "High slippage: expected {}bps, actual {}bps",
                    expected_bps, actual_bps
                )
            }
            RiskFactor::HighTax { tax_bps } => {
                format!(
                    "High token tax: {}bps ({}%)",
                    tax_bps,
                    *tax_bps as f64 / 100.0
                )
            }
            RiskFactor::SandwichTarget { reason } => {
                format!("Sandwich attack target: {}", reason)
            }
            RiskFactor::Honeypot {
                reason,
                buy_success,
                sell_success,
            } => {
                format!(
                    "🚨 HONEYPOT: {} | Buy: {} | Sell: {}",
                    reason,
                    if *buy_success { "✅" } else { "❌" },
                    if *sell_success { "✅" } else { "❌" }
                )
            }
            RiskFactor::UnusualGasPrice { gas_gwei, avg_gwei } => {
                format!("Unusual gas: {} gwei (avg: {} gwei)", gas_gwei, avg_gwei)
            }
            RiskFactor::LargeValue { value_eth } => {
                format!("Large value: {:.4} ETH", value_eth)
            }
            RiskFactor::UnverifiedContract => "Interacting with unverified contract".to_string(),
            RiskFactor::SimulationFailed { reason } => {
                format!("Simulation failed: {}", reason)
            }
            RiskFactor::HighRoundTripTax {
                buy_tax,
                sell_tax,
                total_loss,
            } => {
                format!(
                    "High round-trip tax: Buy {:.2}% + Sell {:.2}% = {:.2}% total loss",
                    buy_tax, sell_tax, total_loss
                )
            }
        }
    }
}

/// Result of transaction analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    /// Transaction hash
    pub tx_hash: B256,
    /// Overall risk level
    pub risk_level: RiskLevel,
    /// List of detected risk factors
    pub risk_factors: Vec<RiskFactor>,
    /// Target address (DEX router)
    pub target: Address,
    /// Sender address
    pub from: Address,
    /// Transaction value in wei
    pub value: U256,
    /// Gas price in wei
    pub gas_price: U256,
    /// Analysis latency in milliseconds
    pub latency_ms: u64,
    /// Timestamp of analysis
    pub timestamp: u64,
}

impl AnalysisResult {
    pub fn new(
        tx_hash: B256,
        from: Address,
        target: Address,
        value: U256,
        gas_price: U256,
    ) -> Self {
        Self {
            tx_hash,
            risk_level: RiskLevel::Safe,
            risk_factors: Vec::new(),
            target,
            from,
            value,
            gas_price,
            latency_ms: 0,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
        }
    }

    /// Add a risk factor and update risk level
    pub fn add_risk(&mut self, factor: RiskFactor) {
        let factor_risk = match &factor {
            RiskFactor::HighSlippage { actual_bps, .. } => {
                if *actual_bps > 1000 {
                    RiskLevel::Critical
                } else if *actual_bps > 500 {
                    RiskLevel::High
                } else {
                    RiskLevel::Medium
                }
            }
            RiskFactor::HighTax { tax_bps } => {
                if *tax_bps > 2000 {
                    RiskLevel::Critical
                } else if *tax_bps > 1000 {
                    RiskLevel::High
                } else {
                    RiskLevel::Medium
                }
            }
            RiskFactor::SandwichTarget { .. } => RiskLevel::Critical,
            RiskFactor::Honeypot { .. } => RiskLevel::Critical,
            RiskFactor::HighRoundTripTax { total_loss, .. } => {
                if *total_loss > 30.0 {
                    RiskLevel::Critical
                } else if *total_loss > 15.0 {
                    RiskLevel::High
                } else {
                    RiskLevel::Medium
                }
            }
            RiskFactor::UnusualGasPrice { .. } => RiskLevel::Medium,
            RiskFactor::LargeValue { .. } => RiskLevel::Low,
            RiskFactor::UnverifiedContract => RiskLevel::Medium,
            RiskFactor::SimulationFailed { .. } => RiskLevel::High,
        };

        // Update to highest risk level
        if (factor_risk as u8) > (self.risk_level as u8) {
            self.risk_level = factor_risk;
        }

        self.risk_factors.push(factor);
    }

    /// Set the analysis latency
    pub fn set_latency(&mut self, start: Instant) {
        self.latency_ms = start.elapsed().as_millis() as u64;
    }

    /// Pretty print the analysis result
    pub fn summary(&self) -> String {
        let mut output = format!(
            "\n{} Risk: {} | TX: {:.8}...\n",
            self.risk_level.emoji(),
            self.risk_level.as_str(),
            hex::encode(self.tx_hash)
        );
        output.push_str(&format!("   From: {}\n", self.from));
        output.push_str(&format!("   To: {}\n", self.target));
        output.push_str(&format!("   Latency: {}ms\n", self.latency_ms));

        if !self.risk_factors.is_empty() {
            output.push_str("   Factors:\n");
            for factor in &self.risk_factors {
                output.push_str(&format!("     - {}\n", factor.description()));
            }
        }

        output
    }
}

/// Parsed swap parameters from DEX calldata
#[derive(Debug, Clone)]
pub struct SwapParams {
    pub amount_in: U256,
    pub amount_out_min: U256,
    pub path: Vec<Address>,
    #[allow(dead_code)]
    pub deadline: U256,
}

/// Statistics for monitoring
#[derive(Debug, Default)]
pub struct SentryStats {
    pub total_received: u64,
    pub total_filtered: u64,
    pub total_analyzed: u64,
    pub total_risky: u64,
    pub avg_latency_ms: f64,
}
