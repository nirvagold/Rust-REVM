//! Telemetry Module for Ruster REVM
//!
//! Collects anonymous statistics about detected threats for:
//! - Marketing reports ("Saved $2M from 500+ honeypots this week")
//! - Performance monitoring
//! - Product improvement insights
//!
//! Privacy-first: No wallet addresses or transaction hashes stored

use alloy_primitives::U256;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// Telemetry event types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ThreatType {
    Honeypot,
    HighSlippage,
    SandwichTarget,
    HighTax,
    UnusualGas,
    LargeValue,
    SimulationFailed,
}

impl ThreatType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ThreatType::Honeypot => "honeypot",
            ThreatType::HighSlippage => "high_slippage",
            ThreatType::SandwichTarget => "sandwich_target",
            ThreatType::HighTax => "high_tax",
            ThreatType::UnusualGas => "unusual_gas",
            ThreatType::LargeValue => "large_value",
            ThreatType::SimulationFailed => "simulation_failed",
        }
    }
}

/// Single telemetry event (anonymized)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryEvent {
    /// Unix timestamp
    pub timestamp: u64,
    /// Type of threat detected
    pub threat_type: ThreatType,
    /// Value at risk in ETH (rounded to hide exact amounts)
    pub value_at_risk_eth: f64,
    /// Detection latency in milliseconds
    pub latency_ms: u64,
    /// Risk level (1-5)
    pub risk_level: u8,
    /// Additional context (no PII)
    pub context: String,
}

impl TelemetryEvent {
    pub fn new(
        threat_type: ThreatType,
        value_wei: U256,
        latency_ms: u64,
        risk_level: u8,
        context: String,
    ) -> Self {
        // Round value to nearest 0.1 ETH for privacy
        let value_eth = wei_to_eth(value_wei);
        let rounded_value = (value_eth * 10.0).round() / 10.0;

        Self {
            timestamp: current_timestamp(),
            threat_type,
            value_at_risk_eth: rounded_value,
            latency_ms,
            risk_level,
            context,
        }
    }
}

/// Aggregated statistics for reporting
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TelemetryStats {
    /// Total transactions analyzed
    pub total_analyzed: u64,
    /// Total threats detected
    pub total_threats: u64,
    /// Threats by type
    pub threats_by_type: HashMap<String, u64>,
    /// Total value protected (ETH)
    pub total_value_protected_eth: f64,
    /// Average detection latency (ms)
    pub avg_latency_ms: f64,
    /// Period start timestamp
    pub period_start: u64,
    /// Period end timestamp
    pub period_end: u64,
    /// Honeypots detected (highlight metric)
    pub honeypots_detected: u64,
    /// Estimated USD saved (at current ETH price)
    pub estimated_usd_saved: f64,
}

impl TelemetryStats {
    /// Generate marketing summary
    pub fn marketing_summary(&self, eth_price_usd: f64) -> String {
        let usd_saved = self.total_value_protected_eth * eth_price_usd;
        let period_hours = (self.period_end - self.period_start) / 3600;

        format!(
            r#"
╔══════════════════════════════════════════════════════════════════╗
║           🛡️ RUSTER REVM - PROTECTION REPORT                     ║
╠══════════════════════════════════════════════════════════════════╣
║                                                                  ║
║   📊 Period: {} hours                                            ║
║                                                                  ║
║   🔍 Transactions Analyzed:    {:>10}                           ║
║   🚨 Threats Detected:         {:>10}                           ║
║   🍯 Honeypots Blocked:        {:>10}                           ║
║                                                                  ║
║   💰 Value Protected:          {:>10.2} ETH                      ║
║   💵 Estimated USD Saved:      ${:>10.0}                         ║
║                                                                  ║
║   ⚡ Avg Detection Latency:    {:>10.2}ms                        ║
║                                                                  ║
╠══════════════════════════════════════════════════════════════════╣
║   "Pre-Execution Risk Scoring for DeFi Protection"               ║
╚══════════════════════════════════════════════════════════════════╝
"#,
            period_hours,
            self.total_analyzed,
            self.total_threats,
            self.honeypots_detected,
            self.total_value_protected_eth,
            usd_saved,
            self.avg_latency_ms,
        )
    }

    /// Export as JSON for API
    #[allow(dead_code)]
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Export as CSV row
    pub fn to_csv_row(&self) -> String {
        format!(
            "{},{},{},{},{:.2},{:.2},{}\n",
            self.period_start,
            self.period_end,
            self.total_analyzed,
            self.total_threats,
            self.total_value_protected_eth,
            self.avg_latency_ms,
            self.honeypots_detected,
        )
    }
}

/// Main telemetry collector
pub struct TelemetryCollector {
    /// Event buffer (in-memory)
    events: Arc<RwLock<Vec<TelemetryEvent>>>,
    /// Atomic counters for fast updates
    total_analyzed: AtomicU64,
    total_threats: AtomicU64,
    honeypots_detected: AtomicU64,
    total_latency_ms: AtomicU64,
    total_value_wei: Arc<RwLock<U256>>,
    /// Threat counters by type
    threat_counts: Arc<RwLock<HashMap<ThreatType, u64>>>,
    /// Session start time
    session_start: u64,
    /// Export directory
    export_dir: PathBuf,
    /// Max events in memory before flush
    max_buffer_size: usize,
}

impl TelemetryCollector {
    /// Create new collector with default settings
    pub fn new() -> Self {
        Self::with_config(PathBuf::from("./telemetry"), 1000)
    }

    /// Create collector with custom config
    pub fn with_config(export_dir: PathBuf, max_buffer_size: usize) -> Self {
        // Ensure export directory exists
        let _ = fs::create_dir_all(&export_dir);

        Self {
            events: Arc::new(RwLock::new(Vec::with_capacity(max_buffer_size))),
            total_analyzed: AtomicU64::new(0),
            total_threats: AtomicU64::new(0),
            honeypots_detected: AtomicU64::new(0),
            total_latency_ms: AtomicU64::new(0),
            total_value_wei: Arc::new(RwLock::new(U256::ZERO)),
            threat_counts: Arc::new(RwLock::new(HashMap::new())),
            session_start: current_timestamp(),
            export_dir,
            max_buffer_size,
        }
    }

    /// Record a transaction analysis (no threat)
    pub fn record_analysis(&self, latency_ms: u64) {
        self.total_analyzed.fetch_add(1, Ordering::Relaxed);
        self.total_latency_ms
            .fetch_add(latency_ms, Ordering::Relaxed);
    }

    /// Record a detected threat
    pub fn record_threat(&self, event: TelemetryEvent) {
        // Update atomic counters
        self.total_analyzed.fetch_add(1, Ordering::Relaxed);
        self.total_threats.fetch_add(1, Ordering::Relaxed);
        self.total_latency_ms
            .fetch_add(event.latency_ms, Ordering::Relaxed);

        // Track honeypots separately (key marketing metric)
        if event.threat_type == ThreatType::Honeypot {
            self.honeypots_detected.fetch_add(1, Ordering::Relaxed);
        }

        // Update value protected
        if let Ok(mut value) = self.total_value_wei.write() {
            let event_value = U256::from((event.value_at_risk_eth * 1e18) as u128);
            *value = value.saturating_add(event_value);
        }

        // Update threat type counter
        if let Ok(mut counts) = self.threat_counts.write() {
            *counts.entry(event.threat_type.clone()).or_insert(0) += 1;
        }

        // Buffer event
        if let Ok(mut events) = self.events.write() {
            events.push(event);

            // Auto-flush if buffer full
            if events.len() >= self.max_buffer_size {
                let events_to_flush = std::mem::take(&mut *events);
                drop(events); // Release lock before I/O
                let _ = self.flush_events(&events_to_flush);
            }
        }
    }

    /// Get current statistics
    pub fn get_stats(&self) -> TelemetryStats {
        let total_analyzed = self.total_analyzed.load(Ordering::Relaxed);
        let total_threats = self.total_threats.load(Ordering::Relaxed);
        let total_latency = self.total_latency_ms.load(Ordering::Relaxed);
        let honeypots = self.honeypots_detected.load(Ordering::Relaxed);

        let avg_latency = if total_analyzed > 0 {
            total_latency as f64 / total_analyzed as f64
        } else {
            0.0
        };

        let value_protected = self
            .total_value_wei
            .read()
            .map(|v| wei_to_eth(*v))
            .unwrap_or(0.0);

        let threats_by_type = self
            .threat_counts
            .read()
            .map(|counts| {
                counts
                    .iter()
                    .map(|(k, v)| (k.as_str().to_string(), *v))
                    .collect()
            })
            .unwrap_or_default();

        TelemetryStats {
            total_analyzed,
            total_threats,
            threats_by_type,
            total_value_protected_eth: value_protected,
            avg_latency_ms: avg_latency,
            period_start: self.session_start,
            period_end: current_timestamp(),
            honeypots_detected: honeypots,
            estimated_usd_saved: 0.0, // Calculated at display time
        }
    }

    /// Export current stats to JSON file
    pub fn export_stats_json(&self) -> Result<PathBuf, std::io::Error> {
        let stats = self.get_stats();
        let filename = format!("stats_{}.json", current_timestamp());
        let path = self.export_dir.join(filename);

        let json = serde_json::to_string_pretty(&stats)?;
        fs::write(&path, json)?;

        Ok(path)
    }

    /// Export stats to CSV (append mode)
    pub fn export_stats_csv(&self) -> Result<PathBuf, std::io::Error> {
        let stats = self.get_stats();
        let path = self.export_dir.join("telemetry_history.csv");

        let mut file = OpenOptions::new().create(true).append(true).open(&path)?;

        // Write header if new file
        if file.metadata()?.len() == 0 {
            writeln!(file, "period_start,period_end,total_analyzed,total_threats,value_protected_eth,avg_latency_ms,honeypots_detected")?;
        }

        write!(file, "{}", stats.to_csv_row())?;

        Ok(path)
    }

    /// Flush buffered events to disk
    fn flush_events(&self, events: &[TelemetryEvent]) -> Result<(), std::io::Error> {
        if events.is_empty() {
            return Ok(());
        }

        let filename = format!("events_{}.jsonl", current_timestamp());
        let path = self.export_dir.join(filename);

        let mut file = OpenOptions::new().create(true).append(true).open(&path)?;

        for event in events {
            if let Ok(json) = serde_json::to_string(event) {
                writeln!(file, "{}", json)?;
            }
        }

        Ok(())
    }

    /// Generate marketing report
    pub fn generate_marketing_report(&self, eth_price_usd: f64) -> String {
        let stats = self.get_stats();
        stats.marketing_summary(eth_price_usd)
    }

    /// Reset counters (for new reporting period)
    #[allow(dead_code)]
    pub fn reset(&self) {
        self.total_analyzed.store(0, Ordering::Relaxed);
        self.total_threats.store(0, Ordering::Relaxed);
        self.honeypots_detected.store(0, Ordering::Relaxed);
        self.total_latency_ms.store(0, Ordering::Relaxed);

        if let Ok(mut value) = self.total_value_wei.write() {
            *value = U256::ZERO;
        }

        if let Ok(mut counts) = self.threat_counts.write() {
            counts.clear();
        }

        if let Ok(mut events) = self.events.write() {
            events.clear();
        }
    }
}

impl Default for TelemetryCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// API-ready telemetry response
#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct TelemetryApiResponse {
    pub success: bool,
    pub data: TelemetryStats,
    pub generated_at: u64,
    pub version: String,
}

#[allow(dead_code)]
impl TelemetryApiResponse {
    pub fn from_stats(stats: TelemetryStats) -> Self {
        Self {
            success: true,
            data: stats,
            generated_at: current_timestamp(),
            version: "0.2.0".to_string(),
        }
    }
}

/// Weekly report generator
#[allow(dead_code)]
pub struct WeeklyReportGenerator {
    collector: Arc<TelemetryCollector>,
}

#[allow(dead_code)]
impl WeeklyReportGenerator {
    pub fn new(collector: Arc<TelemetryCollector>) -> Self {
        Self { collector }
    }

    /// Generate weekly summary for Discord/Telegram
    pub fn generate_social_post(&self, eth_price: f64) -> String {
        let stats = self.collector.get_stats();
        let usd_saved = stats.total_value_protected_eth * eth_price;

        format!(
            r#"🦀 **RUSTER REVM WEEKLY REPORT**

📊 This week we protected:
• 🔍 **{}** transactions analyzed
• 🚨 **{}** threats detected
• 🍯 **{}** honeypots blocked

💰 **{:.1} ETH** (~${:.0}) saved from scams!

⚡ Average PERS detection: **{:.1}ms**

_Pre-Execution Risk Scoring for DeFi Protection_

#DeFi #Security #Rust #REVM"#,
            stats.total_analyzed,
            stats.total_threats,
            stats.honeypots_detected,
            stats.total_value_protected_eth,
            usd_saved,
            stats.avg_latency_ms,
        )
    }
}

// Helper functions

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn wei_to_eth(wei: U256) -> f64 {
    let wei_u128: u128 = wei.try_into().unwrap_or(u128::MAX);
    wei_u128 as f64 / 1e18
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telemetry_event_creation() {
        let event = TelemetryEvent::new(
            ThreatType::Honeypot,
            U256::from(1_500_000_000_000_000_000u128), // 1.5 ETH
            25,
            5,
            "Sell failed".to_string(),
        );

        assert_eq!(event.threat_type, ThreatType::Honeypot);
        assert_eq!(event.value_at_risk_eth, 1.5); // Rounded
        assert_eq!(event.latency_ms, 25);
        assert_eq!(event.risk_level, 5);
    }

    #[test]
    fn test_collector_basic() {
        let collector = TelemetryCollector::new();

        // Record some analyses
        collector.record_analysis(10);
        collector.record_analysis(20);

        // Record a threat
        let event = TelemetryEvent::new(
            ThreatType::Honeypot,
            U256::from(2_000_000_000_000_000_000u128),
            15,
            5,
            "Test".to_string(),
        );
        collector.record_threat(event);

        let stats = collector.get_stats();
        assert_eq!(stats.total_analyzed, 3);
        assert_eq!(stats.total_threats, 1);
        assert_eq!(stats.honeypots_detected, 1);
    }

    #[test]
    fn test_stats_json_export() {
        let stats = TelemetryStats {
            total_analyzed: 1000,
            total_threats: 50,
            honeypots_detected: 25,
            total_value_protected_eth: 150.5,
            avg_latency_ms: 23.5,
            ..Default::default()
        };

        let json = stats.to_json();
        assert!(json.contains("1000"));
        assert!(json.contains("honeypots_detected"));
    }

    #[test]
    fn test_marketing_summary() {
        let stats = TelemetryStats {
            total_analyzed: 50000,
            total_threats: 500,
            honeypots_detected: 150,
            total_value_protected_eth: 250.0,
            avg_latency_ms: 18.5,
            period_start: 1704067200, // Jan 1, 2024
            period_end: 1704672000,   // Jan 8, 2024
            ..Default::default()
        };

        let report = stats.marketing_summary(2500.0); // $2500/ETH
        assert!(report.contains("250.00 ETH"));
        assert!(report.contains("625000")); // USD saved (no comma in Rust format)
        assert!(report.contains("150")); // Honeypots
    }
}
