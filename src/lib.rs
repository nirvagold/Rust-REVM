//! Ruster REVM Library
//!
//! High-performance REVM-based token risk analyzer implementing
//! Pre-Execution Risk Scoring (PERS) for detecting:
//! - Honeypot tokens via simulated Buy-Approve-Sell cycles
//! - High tax tokens (fee-on-transfer)
//! - Sandwich attack targets
//! - MEV exposure risks

pub mod analyzer;
pub mod api;
pub mod config;
pub mod decoder;
pub mod honeypot;
pub mod risk_score;
pub mod simulator;
pub mod telemetry;
pub mod types;

pub use analyzer::MempoolAnalyzer;
pub use config::{DexRouters, SentryConfig};
pub use decoder::SwapDecoder;
pub use honeypot::{HoneypotDetector, HoneypotResult};
pub use risk_score::{RiskComponents, RiskScore, RiskScoreBuilder};
pub use simulator::Simulator;
pub use telemetry::{TelemetryCollector, TelemetryEvent, TelemetryStats, ThreatType};
pub use types::{AnalysisResult, RiskFactor, RiskLevel, SwapParams};
